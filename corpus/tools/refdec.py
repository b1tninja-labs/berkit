"""Reference decoder and encoder for the berkit conformance corpus (stdlib only).

Written from the clauses of X.690 (02/2021) and X.680 (02/2021) cited in the corpus, and from corpus/format.md.
It shares no code with any Rust or Python implementation. It exists so that `check.py` can run every
decode case of the corpus against an independent reading of the rules, and so that tree-form cases can be
re-encoded to bytes.

Rule for several faults in one input: stream order (format.md, "Outcomes"). Tie rules this module adds and
that README.md records: inside an open indefinite-length encoding a lone final `00` is the identifier of an
element whose length octet is missing (`truncated`, or `length_overrun` when the region is the contents of a
definite-length ancestor); a universal tag 0 element that is not the closing EOC of an open indefinite-length
encoding is `stray_eoc`.
"""

from __future__ import annotations

import re

CLASSES = ["universal", "application", "context", "private"]

DEFAULT_LIMITS = {"max_depth": 64, "max_length": 2**64 - 1, "max_tag_number": 2**64 - 1}


class Err(Exception):
    def __init__(self, cls: str, why: str = ""):
        super().__init__(f"{cls}: {why}")
        self.cls = cls
        self.why = why


# ---------------------------------------------------------------- types
# name -> (tag number, kind); kind: prim = always primitive, cons = always constructed,
# str = primitive or constructed at the sender's option in BER (DER: primitive).
UNIVERSAL = {
    "BOOLEAN": (1, "prim"), "INTEGER": (2, "prim"), "BIT STRING": (3, "str"), "OCTET STRING": (4, "str"),
    "NULL": (5, "prim"), "OBJECT IDENTIFIER": (6, "prim"), "REAL": (9, "prim"), "ENUMERATED": (10, "prim"),
    "UTF8String": (12, "str"), "RELATIVE-OID": (13, "prim"), "TIME": (14, "prim"),
    "SEQUENCE": (16, "cons"), "SET": (17, "cons"),
    "NumericString": (18, "str"), "PrintableString": (19, "str"), "TeletexString": (20, "str"),
    "VideotexString": (21, "str"), "IA5String": (22, "str"), "UTCTime": (23, "str"),
    "GeneralizedTime": (24, "str"), "GraphicString": (25, "str"), "VisibleString": (26, "str"),
    "GeneralString": (27, "str"), "UniversalString": (28, "str"), "BMPString": (30, "str"),
    "DATE": (31, "prim"), "TIME-OF-DAY": (32, "prim"), "DATE-TIME": (33, "prim"), "DURATION": (34, "prim"),
    "OID-IRI": (35, "prim"), "RELATIVE-OID-IRI": (36, "prim"),
}

SHARED = {
    "truncated", "reserved_length", "indefinite_on_primitive", "length_overrun", "missing_eoc", "non_minimal_tag",
    "limit_exceeded", "wrong_form", "bad_contents", "non_minimal_integer", "non_minimal_subidentifier",
    "trailing_data", "stray_eoc",
}

DER_ONLY = {
    "non_minimal_length", "indefinite_length", "constructed_string", "boolean_not_ff", "unused_bits_nonzero",
    "bitstring_trailing_zeros", "set_order", "setof_order", "default_encoded", "time_format", "real_form",
}


def resolve_type(t):
    """Return (kind, universal-name-or-None, tag-or-None, extra) for a case `type` string."""
    if t is None:
        return ("none", None, None, None)
    if t == "SET":
        return ("set", "SET", 17, "cons")
    if t in UNIVERSAL:
        n, k = UNIVERSAL[t]
        return ("universal", t, n, k)
    if t.startswith("SEQUENCE"):
        return ("universal", "SEQUENCE", 16, "cons")
    if t == "SET OF" or t.startswith("SET OF "):
        return ("setof", "SET", 17, "cons")
    if t.startswith("SET "):
        return ("set", "SET", 17, "cons")
    if t in ("corpus:NamedBitString",):
        return ("universal", "BIT STRING", 3, "str")
    if t == "x690:A.1:PersonnelRecord":
        return ("personnel", None, None, None)
    if t.startswith("corpus:"):
        return ("corpus", t, None, None)
    return ("opaque", t, None, None)


# ---------------------------------------------------------------- parsing
def _ident(buf, pos, lim):
    if pos >= len(buf):
        raise Err("truncated", "identifier")
    b = buf[pos]
    pos += 1
    cls, cons, num = b >> 6, bool(b & 0x20), b & 0x1F
    nsub = 0
    if num == 31:
        num = 0
        first = True
        while True:
            if pos >= len(buf):
                raise Err("truncated", "high tag number")
            o = buf[pos]
            pos += 1
            nsub += 1
            if first and (o & 0x7F) == 0:
                raise Err("non_minimal_tag", "leading zero group")
            first = False
            num = (num << 7) | (o & 0x7F)
            if num > lim["max_tag_number"]:
                raise Err("limit_exceeded", "tag number")
            if not o & 0x80:
                break
        if num < 31:
            raise Err("non_minimal_tag", "number below 31 in high form")
    return cls, cons, num, pos


def _length(buf, pos, lim):
    if pos >= len(buf):
        raise Err("truncated", "length")
    b = buf[pos]
    pos += 1
    if b < 0x80:
        return "short", None, b, pos
    if b == 0x80:
        return "indefinite", None, None, pos
    if b == 0xFF:
        raise Err("reserved_length")
    k = b & 0x7F
    if pos + k > len(buf):
        raise Err("truncated", "length octets")
    val = int.from_bytes(buf[pos : pos + k], "big")
    pos += k
    if val > lim["max_length"]:
        raise Err("limit_exceeded", "length")
    return "long", k, val, pos


def _minimal_length(form, k, val):
    if form == "short":
        return True
    if val < 128:
        return False
    return k == (val.bit_length() + 7) // 8


class Decoder:
    def __init__(self, buf, mode, lim, top_kind):
        self.buf, self.mode, self.lim = buf, mode, lim
        self.top = top_kind  # (kind, name, tag, extra)

    def element(self, pos, end, bounded, depth, typed):
        buf = self.buf
        cls, cons, num, p = _ident(buf[:end] if False else buf, pos, self.lim)
        if p > end:
            raise Err("length_overrun" if bounded else "truncated", "identifier")
        if depth > self.lim["max_depth"]:
            raise Err("limit_exceeded", "depth")
        if cls == 0 and num == 0:
            raise Err("stray_eoc", "universal tag 0 that is not an EOC")
        kind = typed[0] if typed else "none"
        name, ttag, tk = (typed[1], typed[2], typed[3]) if typed else (None, None, None)
        if kind in ("universal", "set", "setof"):
            if cls != 0 or num != ttag:
                raise Err("tag-mismatch", f"type {name} expects universal {ttag}")
            if tk == "prim" and cons:
                raise Err("wrong_form")
            if tk == "cons" and not cons:
                raise Err("wrong_form")
            if tk == "str" and cons and self.mode == "der":
                raise Err("constructed_string")
        form, k, val, q = self._length_checked(p, end, bounded, cons)
        node = {"class": CLASSES[cls], "constructed": cons, "tag": num, "length": {"form": form}}
        if form == "long":
            node["length"]["octets"] = k
        if form == "indefinite":
            children, q = self._children_indef(q, end, bounded, depth, typed, cls, num)
            node["children"] = children
            return node, q
        if q + val > end:
            raise Err("length_overrun" if bounded else "truncated", "contents")
        cend = q + val
        if cons:
            children = self._children_def(q, cend, depth, typed, cls, num)
            node["children"] = children
        else:
            node["content"] = buf[q:cend].hex()
        return node, cend

    def _length_checked(self, p, end, bounded, cons):
        form, k, val, q = _length(self.buf, p, self.lim)
        if q > end:
            raise Err("length_overrun" if bounded else "truncated", "length octets")
        if form == "indefinite" and not cons:
            raise Err("indefinite_on_primitive")
        if self.mode == "der":
            if form == "indefinite":
                raise Err("indefinite_length")
            if not _minimal_length(form, k, val):
                raise Err("non_minimal_length")
        return form, k, val, q

    def _child_type(self, typed, parent_cls, parent_num):
        return None

    def _children_def(self, pos, end, depth, typed, pcls, pnum):
        out = []
        while pos < end:
            node, pos = self.element(pos, end, True, depth + 1, None)
            out.append(node)
        return out

    def _children_indef(self, pos, end, bounded, depth, typed, pcls, pnum):
        out = []
        while True:
            if pos >= end:
                raise Err("missing_eoc")
            if self.buf[pos] == 0x00:
                if pos + 1 >= end:
                    raise Err("length_overrun" if bounded else "truncated", "a lone 00: the length octet of the EOC is missing")
                if self.buf[pos + 1] == 0x00:
                    return out, pos + 2
                raise Err("stray_eoc", "universal tag 0 with non-zero length")
            node, pos = self.element(pos, end, bounded, depth + 1, None)
            out.append(node)


def decode(hexstr, typ, mode, lim=None):
    """Return the tree of the single top-level encoding, or raise Err."""
    lim = lim or DEFAULT_LIMITS
    buf = bytes.fromhex(hexstr)
    top = resolve_type(typ)
    typed = top if top[0] in ("universal", "set", "setof") else None
    d = Decoder(buf, mode, lim, top)
    if len(buf) >= 1 and buf[0] == 0x00 and len(buf) == 2 and buf[1] == 0:
        raise Err("stray_eoc", "universal tag 0 (stray EOC)")
    node, pos = d.element(0, len(buf), False, 1, typed)
    if pos != len(buf):
        raise Err("trailing_data")
    _typecheck(node, top, mode, typ)
    return node


# ---------------------------------------------------------------- content checks
def _leaf_bytes(node):
    """Concatenated primitive contents of a (possibly constructed) string."""
    if not node["constructed"]:
        return bytes.fromhex(node["content"])
    return b"".join(_leaf_bytes(c) for c in node["children"])


def _leaves(node):
    if not node["constructed"]:
        return [node]
    out = []
    for c in node["children"]:
        out.extend(_leaves(c))
    return out


def _check_segments(node, tag):
    if node["constructed"]:
        for c in node["children"]:
            if c["class"] != "universal" or c["tag"] != tag:
                raise Err("bad_contents", "segment tag")
            _check_segments(c, tag)


def _enc_len_node(node):
    return encode_node(node)


def _typecheck(node, top, mode, typ):
    kind, name, tag, extra = top
    if kind in ("none", "opaque"):
        return
    if kind == "personnel":
        if mode == "der":
            _set_order(node)  # top-level SET components
            for c in node["children"]:
                if c["class"] == "context" and c["tag"] == 3:
                    for s in c["children"]:  # SEQUENCE OF ChildInformation (implicit [3])
                        _set_order(s)
        return
    if kind == "corpus":
        return _corpus_type(node, typ, mode)
    if kind == "set":
        if mode == "der":
            _set_order(node)
        return
    if kind == "setof":
        if mode == "der":
            _setof_order(node)
        return
    # universal
    if name in ("SEQUENCE",):
        return
    if name == "SET":
        return
    if extra == "str":
        if node["constructed"]:
            seg_tag = 3 if name == "BIT STRING" else 4
            _check_segments(node, seg_tag)
        _string_contents(node, name, mode, typ)
        return
    _primitive_contents(node, name, mode)


def _set_order(node):
    prev = None
    for c in node["children"]:
        key = (CLASSES.index(c["class"]), c["tag"])
        if prev is not None and key < prev:
            raise Err("set_order")
        prev = key


def _setof_order(node):
    prev = None
    for c in node["children"]:
        e = encode_node(c)
        if prev is not None:
            n = max(len(prev), len(e))
            a, b = prev.ljust(n, b"\x00"), e.ljust(n, b"\x00")
            if a > b:
                raise Err("setof_order")
        prev = e


def _corpus_type(node, typ, mode):
    if typ == "corpus:ImplicitInt2":
        if node["class"] != "context" or node["tag"] != 2:
            raise Err("tag-mismatch")
        if node["constructed"]:
            raise Err("wrong_form")
        _primitive_contents(node, "INTEGER", mode)
    elif typ == "corpus:ExplicitInt2":
        if node["class"] != "context" or node["tag"] != 2:
            raise Err("tag-mismatch")
        if not node["constructed"]:
            raise Err("wrong_form")
    elif typ == "corpus:DefaultSeq":  # SEQUENCE { a INTEGER DEFAULT 5 }
        if node["tag"] != 16 or node["class"] != "universal":
            raise Err("tag-mismatch")
        for c in node["children"]:
            if c["class"] == "universal" and c["tag"] == 2 and not c["constructed"]:
                _primitive_contents(c, "INTEGER", mode)
                if mode == "der" and int.from_bytes(bytes.fromhex(c["content"]), "big", signed=True) == 5:
                    raise Err("default_encoded")
    elif typ == "corpus:DefaultSeqTagged":  # SEQUENCE { a [0] IMPLICIT INTEGER DEFAULT 5, b [1] IMPLICIT INTEGER OPTIONAL }
        for c in node["children"]:
            if c["class"] == "context" and c["tag"] == 0:
                if mode == "der" and int.from_bytes(bytes.fromhex(c["content"]), "big", signed=True) == 5:
                    raise Err("default_encoded")
    else:
        raise Err("unsupported-type", typ)


def _primitive_contents(node, name, mode):
    c = bytes.fromhex(node["content"])
    if name == "BOOLEAN":
        if len(c) != 1:
            raise Err("bad_contents")
        if mode == "der" and c[0] not in (0, 0xFF):
            raise Err("boolean_not_ff")
    elif name in ("INTEGER", "ENUMERATED"):
        if len(c) == 0:
            raise Err("bad_contents")
        if len(c) > 1 and ((c[0] == 0x00 and c[1] < 0x80) or (c[0] == 0xFF and c[1] >= 0x80)):
            raise Err("non_minimal_integer")
    elif name == "NULL":
        if c:
            raise Err("bad_contents")
    elif name in ("OBJECT IDENTIFIER", "RELATIVE-OID"):
        if not c:
            raise Err("bad_contents", "empty")
        start = True
        for o in c:
            if start and o == 0x80:
                raise Err("non_minimal_subidentifier")
            start = not (o & 0x80)
        if not start:
            raise Err("bad_contents", "subidentifier does not end")
    elif name == "REAL":
        _real(c, mode)
    elif name in ("OID-IRI", "RELATIVE-OID-IRI"):
        _utf8(c)
    elif name in ("TIME", "DATE", "TIME-OF-DAY", "DATE-TIME", "DURATION"):
        _time_types(c, name, mode)
    else:
        raise Err("unsupported-type", name)


def _bitstring_leaf(c):
    if len(c) == 0:
        raise Err("bad_contents", "no initial octet")
    if c[0] > 7:
        raise Err("bad_contents", "unused bits > 7")
    if len(c) == 1 and c[0] != 0:
        raise Err("bad_contents", "unused bits without data")


def _string_contents(node, name, mode, typ):
    if name == "BIT STRING":
        leaves = _leaves(node)
        for i, lf in enumerate(leaves):
            c = bytes.fromhex(lf["content"])
            _bitstring_leaf(c)
            if i < len(leaves) - 1 and c[0] != 0:
                raise Err("bad_contents", "non-final segment not a multiple of 8 bits")
            if mode == "der" and len(c) > 1 and c[-1] & ((1 << c[0]) - 1):
                raise Err("unused_bits_nonzero")
        if mode == "der" and typ == "corpus:NamedBitString":
            c = bytes.fromhex(node["content"])
            if len(c) == 1:
                pass
            else:
                if c[0] == 0 and all(b == 0 for b in c[1:]) or c[-1] >> c[0] & 1 == 0:
                    raise Err("bitstring_trailing_zeros")
        return
    data = _leaf_bytes(node)
    if name == "OCTET STRING":
        return
    if name == "NumericString":
        ok = all(b == 0x20 or 0x30 <= b <= 0x39 for b in data)
    elif name == "PrintableString":
        allowed = set(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 '()+,-./:=?")
        ok = all(b in allowed for b in data)
    elif name == "VisibleString":
        ok = all(0x20 <= b <= 0x7E for b in data)
    elif name == "IA5String":
        ok = all(b <= 0x7F for b in data)
    elif name == "UTF8String":
        _utf8(data)
        ok = True
    elif name == "BMPString":
        if len(data) % 2:
            raise Err("bad_contents", "odd length")
        if data[:2] in (b"\xfe\xff", b"\xff\xfe"):
            raise Err("bad_contents", "signature")
        units = [int.from_bytes(data[i : i + 2], "big") for i in range(0, len(data), 2)]
        ok = not any(0xD800 <= u <= 0xDFFF for u in units)
    elif name == "UniversalString":
        if len(data) % 4:
            raise Err("bad_contents", "length not a multiple of 4")
        ok = True
    elif name == "UTCTime":
        return _utctime(data.decode("latin-1"), mode)
    elif name == "GeneralizedTime":
        return _gentime(data.decode("latin-1"), mode)
    elif name in ("TeletexString", "VideotexString", "GraphicString", "GeneralString"):
        ok = True
    else:
        raise Err("unsupported-type", name)
    if not ok:
        raise Err("bad_contents", "character outside the alphabet")


def _utf8(data):
    # shortest form (X.690 8.23.10); the rest of well-formedness is ISO/IEC 10646 Annex D (not asserted by the corpus)
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        raise Err("bad_contents", "UTF-8")


def _utctime(s, mode):
    m = re.fullmatch(r"(\d\d)(\d\d)(\d\d)(\d\d)(\d\d)(\d\d)?(Z|[+-]\d{4})", s, re.ASCII)
    if not m:
        raise Err("bad_contents", "UTCTime syntax")
    yy, mo, dd, hh, mi, ss = m.groups()[:6]
    if not (1 <= int(mo) <= 12 and 1 <= int(dd) <= 31 and int(hh) <= 23 and int(mi) <= 59 and (ss is None or int(ss) <= 59)):
        raise Err("bad_contents", "UTCTime range")
    if mode == "der" and not re.fullmatch(r"\d{12}Z", s):
        raise Err("time_format")


def _gentime(s, mode):
    m = re.fullmatch(r"(\d{4})(\d\d)(\d\d)(\d\d)(\d\d)?(\d\d)?(?:[.,](\d+))?(Z|[+-]\d\d(?:\d\d)?)?", s, re.ASCII)
    if not m or (m.group(6) and not m.group(5)):
        raise Err("bad_contents", "GeneralizedTime syntax")
    yyyy, mo, dd, hh, mi, ss, frac, zone = m.groups()
    if not (1 <= int(mo) <= 12 and 1 <= int(dd) <= 31 and int(hh) <= 23 and (mi is None or int(mi) <= 59) and (ss is None or int(ss) <= 59)):
        raise Err("bad_contents", "GeneralizedTime range")
    if mode == "der" and not re.fullmatch(r"\d{14}(\.\d*[1-9])?Z", s):
        raise Err("time_format")


def _time_types(c, name, mode):
    try:
        s = c.decode("ascii")
    except UnicodeDecodeError:
        raise Err("bad_contents", "non-ASCII")
    if not all(32 <= ord(ch) <= 126 for ch in s):
        raise Err("bad_contents", "non-printable")
    if name == "DATE" and not re.fullmatch(r"\d{8}", s):
        raise Err("bad_contents")
    if name == "TIME-OF-DAY" and not re.fullmatch(r"\d{6}", s):
        raise Err("bad_contents")
    if name == "DATE-TIME" and not re.fullmatch(r"\d{14}", s):
        raise Err("bad_contents")
    if name == "DURATION":
        if not re.fullmatch(r"(\d+Y)?(\d+M)?(\d+D)?(T(\d+H)?(\d+M)?(\d+(\.\d+)?S)?)?", s) and not re.fullmatch(
            r"\d+(\.\d+)?[YMDHS]|(\d+M)?T\d+(\.\d+)?S|\d+MT\d+(\.\d+)?S", s
        ):
            raise Err("bad_contents")
        if mode == "der":
            comps = re.findall(r"(\d+(?:\.\d+)?)[YMDHS]", s)
            if any(float(x) == 0 for x in comps[:-1]):
                raise Err("time_format")
    if name == "TIME" and mode == "der":
        if "," in s or re.search(r"[+-]\d\d:00$", s):
            raise Err("time_format")


def _real(c, mode):
    if len(c) == 0:
        return
    f = c[0]
    if f & 0x80:  # binary
        base = (f >> 4) & 3
        if base == 3:
            raise Err("bad_contents", "reserved base")
        ff = (f >> 2) & 3
        fmt = f & 3
        if fmt == 3:
            if len(c) < 2:
                raise Err("bad_contents")
            x = c[1]
            if x == 0:
                raise Err("bad_contents", "exponent length 0")
            if len(c) < 2 + x:
                raise Err("bad_contents")
            e = c[2 : 2 + x]
            n = c[2 + x :]
            if x >= 2:
                bits = format(int.from_bytes(e[:2], "big"), "016b")[:9]
                if bits in ("000000000", "111111111"):
                    raise Err("bad_contents", "exponent first nine bits equal")
        else:
            elen = fmt + 1
            if len(c) < 1 + elen:
                raise Err("bad_contents")
            e = c[1 : 1 + elen]
            n = c[1 + elen :]
        if mode == "der":
            ev = int.from_bytes(e, "big", signed=True)
            need = 1
            while not (-(1 << (8 * need - 1)) <= ev < (1 << (8 * need - 1))):
                need += 1
            wantfmt = need - 1 if need <= 3 else 3
            if base != 0 or ff != 0 or len(e) != need or fmt != wantfmt or (n and (n[-1] & 1) == 0) or (n and n[0] == 0):
                raise Err("real_form")
    elif f & 0x40:
        if len(c) != 1 or f not in (0x40, 0x41, 0x42, 0x43):
            raise Err("bad_contents", "special value")
    else:
        nr = f & 0x3F
        if nr not in (1, 2, 3):
            raise Err("bad_contents", "reserved decimal form")
        if mode == "der" and nr != 3:
            raise Err("real_form")


# ---------------------------------------------------------------- encoder
def _enc_id(cls, cons, num):
    b = (CLASSES.index(cls) << 6) | (0x20 if cons else 0)
    if num < 31:
        return bytes([b | num])
    groups = []
    n = num
    while True:
        groups.append(n & 0x7F)
        n >>= 7
        if not n:
            break
    groups.reverse()
    return bytes([b | 31]) + bytes((g | 0x80) for g in groups[:-1]) + bytes([groups[-1]])


def encode_node(node):
    cons = node["constructed"]
    if cons:
        body = b"".join(encode_node(c) for c in node["children"])
    else:
        body = bytes.fromhex(node["content"])
    ln = node["length"]
    form = ln["form"]
    out = _enc_id(node["class"], cons, node["tag"])
    if form == "indefinite":
        return out + b"\x80" + body + b"\x00\x00"
    n = len(body)
    if form == "short":
        if n > 127:
            raise ValueError("short form cannot hold the length")
        return out + bytes([n]) + body
    k = ln["octets"]
    if n.bit_length() > 8 * k:
        raise ValueError("long form octets too few")
    return out + bytes([0x80 | k]) + n.to_bytes(k, "big") + body


def minimal_lengths(node):
    """True when every node of the tree is DER-minimal as far as its length form goes."""
    ln = node["length"]
    if ln["form"] == "indefinite":
        return False
    n = len(encode_node(node)) - len(_enc_id(node["class"], node["constructed"], node["tag"]))
    # recompute content length
    body = b"".join(encode_node(c) for c in node["children"]) if node["constructed"] else bytes.fromhex(node["content"])
    n = len(body)
    if n < 128:
        if ln["form"] != "short":
            return False
    else:
        if ln["form"] != "long" or ln["octets"] != (n.bit_length() + 7) // 8:
            return False
    return all(minimal_lengths(c) for c in node.get("children", []))


# typed encoders for `encode` cases with input.typed
def _der_tlv(tag, body, cons=False, cls=0):
    n = len(body)
    ln = bytes([n]) if n < 128 else bytes([0x80 | ((n.bit_length() + 7) // 8)]) + n.to_bytes((n.bit_length() + 7) // 8, "big")
    return _enc_id(CLASSES[cls], cons, tag) + ln + body


def _int_bytes(v):
    n = 1
    while not (-(1 << (8 * n - 1)) <= v < (1 << (8 * n - 1))):
        n += 1
    return v.to_bytes(n, "big", signed=True)


def _base128(v):
    g = [v & 0x7F]
    v >>= 7
    while v:
        g.append(0x80 | (v & 0x7F))
        v >>= 7
    return bytes(reversed(g))


def encode_typed(typ, value):
    if typ == "BOOLEAN":
        return _der_tlv(1, b"\xff" if value else b"\x00")
    if typ in ("INTEGER", "ENUMERATED"):
        return _der_tlv(2 if typ == "INTEGER" else 10, _int_bytes(int(value)))
    if typ == "NULL":
        return _der_tlv(5, b"")
    if typ == "OBJECT IDENTIFIER":
        a = [int(x) for x in value.split(".")]
        body = _base128(a[0] * 40 + a[1]) + b"".join(_base128(x) for x in a[2:])
        return _der_tlv(6, body)
    if typ == "RELATIVE-OID":
        return _der_tlv(13, b"".join(_base128(int(x)) for x in value.split(".")))
    if typ == "OCTET STRING":
        return _der_tlv(4, bytes.fromhex(value))
    if typ == "BIT STRING":
        return _der_tlv(3, bytes([value["unused"]]) + bytes.fromhex(value["hex"]))
    tags = {"UTF8String": 12, "NumericString": 18, "PrintableString": 19, "IA5String": 22, "VisibleString": 26}
    if typ in tags:
        return _der_tlv(tags[typ], value.encode("utf-8"))
    raise ValueError(f"no typed encoder for {typ}")
