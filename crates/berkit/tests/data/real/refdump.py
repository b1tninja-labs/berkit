"""Independent reference TLV walker for X.690 DER. Standard library only.

Written for the real-certificate check; it imports nothing from berkit or asn1core and shares no code with them.

    python refdump.py dump FILE...        print one line per node: depth class number form length header-octets
    python refdump.py expect DIR          write DIR/*.tree for every DIR/*.der, and DIR/mutations.expected

The rules are the standard's, read directly:
  X.690 8.1.2   identifier: bits 8-7 class, bit 6 form, bits 5-1 number, 31 = high-tag-number form
  X.690 8.1.3   length: short form < 128; long form 0x80|n then n octets; 0x80 alone is indefinite
  X.690 10.1    DER: definite length, in the fewest octets (8.1.3.5 allows more in BER)
  X.690 11.5    DER: a DEFAULT value is not encoded
  X.690 11.6    DER: SET OF encodings in ascending order, shorter ones padded with trailing zero octets
  X.690 8.3.2   INTEGER: not 00 followed by a byte with bit 8 clear, nor FF followed by a byte with bit 8 set
  X.690 11.1    DER: BOOLEAN TRUE is the single octet FF
A node is descended when its constructed bit is set; primitive nodes (BIT STRING and OCTET STRING included)
are leaves.

The `expect` mode also applies a fixed list of mutations to each certificate and records, for each, the class
this file's reading of the clauses predicts and the sha256 of the mutated bytes, so that the Rust test can
check it built the same bytes before it compares verdicts.
"""

from __future__ import annotations

import hashlib
import os
import sys

CLASS = "UACP"


class Bad(Exception):
    pass


class Node:
    def __init__(self, depth, ident, lenoct, content, enc):
        self.depth = depth
        self.ident = ident
        self.lenoct = lenoct
        b = ident[0]
        self.cls = b >> 6
        self.constructed = bool(b & 0x20)
        self.number = b & 0x1F
        if self.number == 31:
            self.number = 0
            for o in ident[1:]:
                self.number = (self.number << 7) | (o & 0x7F)
        self.length = len(content)
        self.content = content
        self.enc = enc
        self.kids = []


def parse(buf, pos, end, depth):
    """One TLV at buf[pos:end]. Returns (Node, new_pos)."""
    start = pos
    if pos >= end:
        raise Bad("truncated")
    pos += 1
    if buf[start] & 0x1F == 31:
        while True:
            if pos >= end:
                raise Bad("truncated")
            o = buf[pos]
            pos += 1
            if not o & 0x80:
                break
    ident = buf[start:pos]
    if pos >= end:
        raise Bad("truncated")
    first = buf[pos]
    lstart = pos
    pos += 1
    if first < 0x80:
        length = first
    elif first == 0x80:
        raise Bad("indefinite")
    elif first == 0xFF:
        raise Bad("reserved_length")
    else:
        n = first & 0x7F
        if pos + n > end:
            raise Bad("truncated")
        length = int.from_bytes(buf[pos:pos + n], "big")
        pos += n
    lenoct = buf[lstart:pos]
    if pos + length > end:
        raise Bad("length_overrun" if depth else "truncated")
    node = Node(depth, ident, lenoct, buf[pos:pos + length], buf[start:pos + length])
    if node.constructed:
        p = pos
        while p < pos + length:
            kid, p = parse(buf, p, pos + length, depth + 1)
            node.kids.append(kid)
    return node, pos + length


def walk(node):
    yield node
    for k in node.kids:
        yield from walk(k)


def line(n):
    return f"{n.depth} {CLASS[n.cls]} {n.number} {'C' if n.constructed else 'P'} {n.length} {len(n.ident) + len(n.lenoct)}"


def tree(buf):
    root, end = parse(buf, 0, len(buf), 0)
    if end != len(buf):
        raise Bad("trailing_data")
    return root


def enc_len(n, pad=0):
    """DER length octets (X.690 8.1.3, 10.1), with `pad` extra leading zero octets for a mutation."""
    if n < 128 and not pad:
        return bytes([n])
    body = b"\0" * pad + n.to_bytes((n.bit_length() + 7) // 8, "big")
    return bytes([0x80 | len(body)]) + body


def encode(node, edit):
    """Re-encode in DER from the tree; edit(node, content) may return (ident, lenoct, content) to replace one node."""
    if node.kids:
        content = b"".join(encode(k, edit) for k in node.kids)
    else:
        content = node.content
    r = edit(node, content)
    if r is not None:
        ident, lenoct, content = r
        return ident + lenoct + content
    return node.ident + enc_len(len(content)) + content


def only(target, ident, lenoct, content):
    """An edit that replaces exactly `target`."""

    def e(n, c):
        return (ident, lenoct, content) if n is target else None

    return e


def mutations(buf):
    """Yield (name, mutated bytes, predicted DER class or None for valid)."""
    root = tree(buf)
    out = []
    ident = root.ident
    out.append(("len-pad-outer", ident + enc_len(root.length, pad=1) + root.content, "non_minimal_length"))
    t = root.kids[0].kids  # TBSCertificate components
    tbs = root.kids[0]
    out.append(("len-pad-tbs", encode(root, only(tbs, tbs.ident, enc_len(tbs.length, pad=1), tbs.content)),
                "non_minimal_length"))
    small = next(n for n in walk(root) if n.length < 128 and n.depth > 0)
    out.append(("len-81-short", encode(root, only(small, small.ident, bytes([0x81, small.length]), small.content)),
                "non_minimal_length"))
    out.append(("indef-outer", ident + b"\x80" + root.content + b"\0\0", "indefinite_length"))
    out.append(("trailing", buf + b"\0", "trailing_data"))
    out.append(("truncated", buf[:-1], "truncated"))
    has_ver = t[0].cls == 2 and t[0].number == 0
    ser = t[1] if has_ver else t[0]
    if ser.content[0] < 0x80:  # 8.3.2: 00 then a byte with bit 8 clear is not minimal
        c = b"\0" + ser.content
        out.append(("serial-pad00", encode(root, only(ser, ser.ident, enc_len(len(c)), c)), "non_minimal_integer"))
    if has_ver:  # 11.5: version v1 is the DEFAULT, so it is left out
        ver = t[0].kids[0]
        out.append(("version-default", encode(root, only(ver, ver.ident, b"\x01", b"\x00")), "default_encoded"))
    if t[-1].cls == 2 and t[-1].number == 3:
        crit = next((x.kids[1] for x in t[-1].kids[0].kids
                     if len(x.kids) == 3 and x.kids[1].cls == 0 and x.kids[1].number == 1), None)
        if crit is not None:
            out.append(("bool-true-01", encode(root, only(crit, crit.ident, b"\x01", b"\x01")), "boolean_not_ff"))
            out.append(("critical-default-false", encode(root, only(crit, crit.ident, b"\x01", b"\x00")),
                        "default_encoded"))
    issuer = t[3 if has_ver else 2]
    rdn = issuer.kids[0]
    atv = rdn.kids[0].enc
    lower = atv[:-1] + bytes([atv[-1] - 1])  # same length, differs in the last octet, so it sorts first (11.6)
    for name, c, cls in (("setof-descending", atv + lower, "setof_order"), ("setof-ascending", lower + atv, None)):
        out.append((name, encode(root, only(rdn, rdn.ident, enc_len(len(c)), c)), cls))
    return out


def main(argv):
    if len(argv) < 3:
        sys.exit(__doc__)
    if argv[1] == "dump":
        for f in argv[2:]:
            for n in walk(tree(open(f, "rb").read())):
                print(line(n))
    elif argv[1] == "expect":
        d = argv[2]
        rows = []
        for f in sorted(os.listdir(d)):
            if not f.endswith(".der"):
                continue
            buf = open(os.path.join(d, f), "rb").read()
            with open(os.path.join(d, f[:-4] + ".tree"), "w", newline="\n") as o:
                o.write("".join(line(n) + "\n" for n in walk(tree(buf))))
            for name, mb, cls in mutations(buf):
                rows.append(f"{f}\t{name}\t{cls or 'ok'}\t{hashlib.sha256(mb).hexdigest()}")
        with open(os.path.join(d, "mutations.expected"), "w", newline="\n") as o:
            o.write("\n".join(rows) + "\n")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
