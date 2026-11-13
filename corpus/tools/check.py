"""Validate and self-check the berkit conformance corpus. Standard library only.

    python corpus/tools/check.py                    check everything, exit 1 on any failure
    python corpus/tools/check.py --write-manifest   recompute the sha256 of each case file into corpus.json first

Three layers:
  1. schema rules of case.schema.json and format.md, implemented by hand (no jsonschema);
  2. manifest rules: file list, sha256, the 24 error classes, limits, policy keys;
  3. semantic self-checks that use only refdec.py (written from the clauses, independent of any Rust code):
     tree-form cases are re-encoded to bytes and compared with the hex; DER outcomes must have minimal lengths;
     DER-only error classes may not appear under `ber` or `both`; every decode/roundtrip case is run through the
     reference decoder (in each mode it names, with the manifest limits) and must give the stated outcome;
     every encode case is reproduced by the reference encoder and the output is decoded back.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import refdec  # noqa: E402
from refdec import Err  # noqa: E402

HEX = re.compile(r"^([0-9a-f]{2})*$")
ID = re.compile(r"^[a-z0-9]+(\.[a-z0-9-]+)+$")
CLAUSE = re.compile(r"^X\.(690|680) \((02/2021|09/2021)\) §[A-Z0-9][A-Za-z0-9.() ]*$")
PROV = re.compile(r"^(from X\.(690|680) .+|constructed by hand from .+)$")
EDITION = re.compile(r"^X\.(690|680):2021-02(\+E1)?$")
CLASSES = ["universal", "application", "context", "private"]
OPS = ["decode", "encode", "roundtrip"]
# classes that need a `type` (format.md); bad_contents is exempt for a stray EOC (universal tag 0)
NEEDS_TYPE = {"wrong_form", "bad_contents", "constructed_string", "boolean_not_ff", "unused_bits_nonzero",
              "bitstring_trailing_zeros", "set_order", "setof_order", "default_encoded", "time_format", "real_form",
              "non_minimal_integer", "non_minimal_subidentifier"}

fails: list[str] = []


def fail(cid, msg):
    fails.append(f"{cid}: {msg}")


def check_node(n, where, cid):
    if not isinstance(n, dict):
        return fail(cid, f"{where}: node is not an object")
    allowed = {"class", "constructed", "tag", "length", "content", "children", "asn1"}
    for k in n:
        if k not in allowed:
            fail(cid, f"{where}: unknown node key {k}")
    for k in ("class", "constructed", "tag", "length"):
        if k not in n:
            return fail(cid, f"{where}: node lacks {k}")
    if n["class"] not in CLASSES:
        fail(cid, f"{where}: bad class")
    if not isinstance(n["constructed"], bool):
        fail(cid, f"{where}: constructed is not a boolean")
    if not isinstance(n["tag"], int) or isinstance(n["tag"], bool) or n["tag"] < 0:
        fail(cid, f"{where}: bad tag")
    ln = n["length"]
    if not isinstance(ln, dict) or ln.get("form") not in ("short", "long", "indefinite") or set(ln) - {"form", "octets"}:
        return fail(cid, f"{where}: bad length")
    if ln["form"] == "long":
        o = ln.get("octets")
        if not isinstance(o, int) or isinstance(o, bool) or not 1 <= o <= 126:
            fail(cid, f"{where}: long form needs octets 1..126")
    elif "octets" in ln:
        fail(cid, f"{where}: octets only with form long")
    if n["constructed"]:
        if "children" not in n or "content" in n:
            return fail(cid, f"{where}: constructed node needs children and no content")
        for i, c in enumerate(n["children"]):
            check_node(c, f"{where}/{i}", cid)
    else:
        if "content" not in n or "children" in n:
            return fail(cid, f"{where}: primitive node needs content and no children")
        if not (isinstance(n["content"], str) and HEX.match(n["content"])):
            fail(cid, f"{where}: content is not lowercase even-length hex")


def strip(n):
    """A tree without `length` and `asn1`, for the BER roundtrip comparison."""
    o = {k: v for k, v in n.items() if k not in ("length", "asn1", "children")}
    if "children" in n:
        o["children"] = [strip(c) for c in n["children"]]
    return o


def main():
    write = "--write-manifest" in sys.argv
    mpath = os.path.join(CORPUS, "corpus.json")
    manifest = json.load(open(mpath, encoding="utf8"))
    classes = manifest.get("error_classes", [])
    if len(classes) != 24 or len(set(classes)) != 24:
        fail("corpus.json", "error_classes must list 24 distinct classes")
    der_only = set(manifest.get("der_only_error_classes", []))
    if der_only != refdec.DER_ONLY:
        fail("corpus.json", "der_only_error_classes differs from the DER-only classes of format.md")
    if set(classes) != refdec.SHARED | refdec.DER_ONLY or refdec.SHARED & refdec.DER_ONLY or len(refdec.SHARED) != 13:
        fail("corpus.json", "error_classes must be the 13 shared classes plus the 11 DER-only classes")
    lim = manifest.get("limits", {})
    limits = {
        "max_depth": int(lim.get("max_depth", 0)),
        "max_length": int(lim.get("max_length", 0)),
        "max_tag_number": int(lim.get("max_tag_number", 0)),
    }
    if not all(limits.values()):
        fail("corpus.json", "limits must give max_depth, max_length and max_tag_number")
    policies = set(manifest.get("policies", {}))

    files = manifest.get("files", {})
    cdir = os.path.join(CORPUS, "cases")
    on_disk = sorted(f for f in os.listdir(cdir) if f.endswith(".json"))
    if write:
        for name in on_disk:
            files["cases/" + name] = hashlib.sha256(open(os.path.join(cdir, name), "rb").read()).hexdigest()
        manifest["files"] = files = dict(sorted(files.items()))
        with open(mpath, "w", encoding="utf8", newline="\n") as f:
            json.dump(manifest, f, indent=2, ensure_ascii=False)
            f.write("\n")
    if sorted(os.path.basename(k) for k in files) != on_disk:
        fail("corpus.json", f"files listed {sorted(files)} differ from cases/ on disk {on_disk}")
    for rel, want in files.items():
        got = hashlib.sha256(open(os.path.join(CORPUS, rel), "rb").read()).hexdigest()
        if got != want:
            fail(rel, f"sha256 {got} differs from the manifest {want}")

    seen = {}
    per_file = {}
    per_class = Counter()
    per_prov = Counter()
    per_policy = Counter()
    per_op = Counter()
    ncheck = Counter()
    for name in on_disk:
        doc = json.load(open(os.path.join(cdir, name), encoding="utf8"))
        if set(doc) != {"corpus_format", "cases"} or doc["corpus_format"] != "1":
            fail(name, "file must be {corpus_format: '1', cases: [...]}")
        per_file[name] = len(doc["cases"])
        for c in doc["cases"]:
            cid = c.get("id", f"<{name} without id>")
            check_case(c, cid, name, seen, limits, classes, policies, der_only, per_class, per_prov, per_policy, per_op, ncheck)

    print("cases per file:")
    for k, v in per_file.items():
        print(f"  {k:22s} {v}")
    print(f"  {'total':22s} {sum(per_file.values())}")
    print("cases per op:", dict(per_op))
    print("outcomes per error class (a case with ber and der outcomes counts each):")
    for k in classes:
        print(f"  {k:28s} {per_class.get(k, 0)}")
    print("cases per provenance:", dict(per_prov))
    print("cases per policy key:", dict(sorted(per_policy.items())))
    print("semantic checks run:", dict(ncheck))
    if fails:
        print(f"\nFAILED: {len(fails)} problem(s)")
        for f in fails[:200]:
            print("  " + f)
        return 1
    print("\nOK: no problems")
    return 0


def check_case(c, cid, fname, seen, limits, classes, policies, der_only, per_class, per_prov, per_policy, per_op, ncheck):
    allowed = {"id", "title", "op", "type", "input", "expect", "clause", "provenance", "edition", "policy"}
    for k in c:
        if k not in allowed:
            fail(cid, f"unknown field {k}")
    for k in ("id", "op", "input", "expect", "clause", "provenance", "edition"):
        if k not in c:
            fail(cid, f"missing {k}")
    if not isinstance(c.get("id"), str) or not ID.match(c["id"]):
        fail(cid, "id does not match the id pattern")
    if cid in seen:
        fail(cid, f"id reused (also in {seen[cid]})")
    seen[cid] = fname
    op = c.get("op")
    if op not in OPS:
        fail(cid, "bad op")
    per_op[op] += 1
    typ = c.get("type")
    if "type" in c and typ is not None and not isinstance(typ, str):
        fail(cid, "type must be a string or null")
    if "title" in c and not isinstance(c["title"], str):
        fail(cid, "title must be a string")
    if c.get("edition") is None or not EDITION.match(str(c["edition"])):
        fail(cid, "bad edition")
    pv = c.get("provenance", "")
    if not PROV.match(pv):
        fail(cid, "provenance must start 'from X.690/X.680 ...' or 'constructed by hand from ...'")
    per_prov["from X.690" if pv.startswith("from X.690") else "from X.680" if pv.startswith("from X.680") else "constructed by hand"] += 1
    cls = c.get("clause")
    if not isinstance(cls, list) or not cls or not all(isinstance(x, str) and CLAUSE.match(x) for x in cls):
        fail(cid, "clause must be a non-empty list of 'X.690 (02/2021) §n' citations")
    if "policy" in c:
        if c["policy"] not in policies:
            fail(cid, f"policy {c['policy']!r} is not a key of corpus.json policies")
        per_policy[c["policy"]] += 1

    inp = c.get("input", {})
    if not isinstance(inp, dict) or len(inp) != 1 or not set(inp) <= {"hex", "tree", "typed"}:
        return fail(cid, "input must have exactly one of hex, tree, typed")
    if "hex" in inp and not (isinstance(inp["hex"], str) and HEX.match(inp["hex"])):
        fail(cid, "input.hex is not lowercase even-length hex")
    if "tree" in inp:
        check_node(inp["tree"], "input.tree", cid)
    if "typed" in inp:
        t = inp["typed"]
        if not isinstance(t, dict) or set(t) != {"type", "value"}:
            fail(cid, "input.typed must be {type, value}")
    if op in ("decode", "roundtrip") and "hex" not in inp:
        fail(cid, f"{op} needs input.hex")
    if op == "encode" and "tree" not in inp and "typed" not in inp:
        fail(cid, "encode needs input.tree or input.typed")

    exp = c.get("expect", {})
    keys = set(exp) if isinstance(exp, dict) else set()
    if not keys or not keys <= {"both", "ber", "der"} or ("both" in keys and len(keys) > 1):
        return fail(cid, "expect keys must be 'both' alone, or ber and/or der")
    outs = {}
    for k in keys:
        o = exp[k]
        if not isinstance(o, dict) or len(o) != 1 or not set(o) <= {"tree", "hex", "error"}:
            fail(cid, f"expect.{k} must have exactly one of tree, hex, error")
            continue
        (kind, val), = o.items()
        if kind == "error":
            if val not in classes:
                fail(cid, f"expect.{k}.error {val!r} is not one of the 24 classes")
            per_class[val] += 1
        elif kind == "tree":
            check_node(val, f"expect.{k}.tree", cid)
        elif not (isinstance(val, str) and HEX.match(val)):
            fail(cid, f"expect.{k}.hex is not lowercase even-length hex")
        if op == "encode" and kind != "hex":
            fail(cid, "encode outcomes are hex")
        if op != "encode" and kind == "hex":
            fail(cid, f"{op} outcomes are tree or error")
        outs[k] = (kind, val)
        if kind == "error" and k in ("ber", "both") and val in der_only:
            fail(cid, f"DER-only class {val} is stated under {k}; it belongs under der")
        if kind == "error" and val in NEEDS_TYPE and typ is None:
            fail(cid, f"class {val} needs a type")
    if fails and any(f.startswith(cid + ":") for f in fails):
        return

    modes = ["ber", "der"] if "both" in keys else sorted(keys)
    # ---- semantic checks
    if op in ("decode", "roundtrip"):
        for m in modes:
            kind, val = outs["both" if "both" in keys else m]
            try:
                tree = refdec.decode(inp["hex"], typ, m, limits)
                got = ("tree", tree)
            except Err as e:
                got = ("error", e.cls)
            ncheck["reference decode"] += 1
            if got[0] != kind:
                fail(cid, f"{m}: reference decoder gives {got}, the case states {kind} {val if kind == 'error' else ''}")
                continue
            if kind == "error":
                if got[1] != val:
                    fail(cid, f"{m}: reference decoder gives {got[1]}, the case states {val}")
                continue
            if got[1] != val and strip_asn1(got[1]) != strip_asn1(val):
                fail(cid, f"{m}: tree differs from the reference decoder's tree")
            re_enc = refdec.encode_node(val).hex()
            ncheck["tree re-encode"] += 1
            if re_enc != inp["hex"]:
                fail(cid, f"{m}: re-encoding the tree gives {re_enc[:80]}, the input is {inp['hex'][:80]}")
            if m == "der":
                ncheck["DER minimal lengths"] += 1
                if not refdec.minimal_lengths(val):
                    fail(cid, "der: tree has a length that is not minimal")
            if op == "roundtrip":
                ncheck["roundtrip"] += 1
                if m == "der" and re_enc != inp["hex"]:
                    fail(cid, "der roundtrip does not reproduce the input")
                try:
                    again = refdec.decode(re_enc, typ, m, limits)
                    if strip(again) != strip(val):
                        fail(cid, f"{m}: roundtrip tree differs")
                except Err as e:
                    fail(cid, f"{m}: re-encoded bytes do not decode: {e}")
    else:  # encode
        hx_expect = {m: outs["both" if "both" in keys else m][1] for m in modes}
        if "tree" in inp:
            try:
                got = refdec.encode_node(inp["tree"]).hex()
            except ValueError as e:
                return fail(cid, f"tree cannot be encoded: {e}")
            ncheck["tree encode"] += 1
        else:
            try:
                got = refdec.encode_typed(inp["typed"]["type"], inp["typed"]["value"]).hex()
            except ValueError as e:
                return fail(cid, f"typed value cannot be encoded: {e}")
            ncheck["typed encode"] += 1
        for m, hx in hx_expect.items():
            if got != hx:
                fail(cid, f"{m}: reference encoder gives {got[:80]}, the case states {hx[:80]}")
            try:
                back = refdec.decode(hx, typ, m, limits)
                ncheck["encode decodes back"] += 1
                if "tree" in inp and strip(back) != strip(inp["tree"]):
                    fail(cid, f"{m}: encoded bytes decode to a different tree")
            except Err as e:
                fail(cid, f"{m}: expected bytes do not decode: {e}")
            if m == "der" and "tree" in inp and not refdec.minimal_lengths(inp["tree"]):
                fail(cid, "der: input tree has a length that is not minimal")


def strip_asn1(n):
    o = {k: v for k, v in n.items() if k != "asn1"}
    if "children" in o:
        o["children"] = [strip_asn1(c) for c in o["children"]]
    return o


def _why(c, k, limits):
    m = "ber" if k in ("both", "ber") else "der"
    try:
        refdec.decode(c["input"]["hex"], c.get("type"), m, limits)
    except Err as e:
        return e.why
    return ""


if __name__ == "__main__":
    sys.exit(main())
