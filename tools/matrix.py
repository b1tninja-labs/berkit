#!/usr/bin/env python3
"""Generate docs/matrix.md, a reference matrix built from the ROADMAP tables.

    python tools/matrix.py           write docs/matrix.md
    python tools/matrix.py --check   exit 1 if docs/matrix.md is not what this would write

Standard library only. Every cell of the matrix is copied from a ROADMAP row; the only things computed are
counts and the split of an Anchor cell into (document, clause) pairs. A row that cites two clauses appears
under both. A row whose Anchor has no recognisable clause goes to the "Unanchored" section.

Source tables: ROADMAP.md at the root, and every fragment under crates/*/roadmap/*.md and crates/*/ROADMAP.md.
A table is read when its header is `Status | Obligation | Anchor | Test` (or the `Status | Policy | Why` table
of the policy decisions, which has no Test column).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "matrix.md"

# A clause with no document named before it in the same cell is read as this document. Rows that needed it
# carry a dagger in the clause cell.
DEFAULT_DOC = "X.690"
ROOT_MODULE = "(across the modules)"

STATUSES = ("ok", "unread", "open")

# ---------------------------------------------------------------------------------------------------------
# Anchor parsing
# ---------------------------------------------------------------------------------------------------------

# Document names, derived from the Anchor column of the existing rows.
DOC_PATTERNS = [
    (r"X\.(?:690|680|660)", lambda m: m.group(0)),
    (r"RFC\s*(\d+)", lambda m: f"RFC {m.group(1)}"),
    (r"ISO/IEC\s+(\d+)(?:-(\d+))?(?::(\d{4}))?", lambda m: iso_name(m.group(1), m.group(2), m.group(3))),
    (r"ISO\s+(\d+)", lambda m: f"ISO {m.group(1)}"),
    (r"(?<![\w/-])7816-(\d):(\d{4})", lambda m: f"ISO/IEC 7816-{m.group(1)}:{m.group(2)}"),
    (r"SP\s+800-73-5\s+Part\s+(\d)", lambda m: f"SP 800-73-5 Part {m.group(1)}"),
    (r"SP\s+800-78-5", lambda m: "SP 800-78-5"),
    (r"FIPS\s+[\d-]+", lambda m: re.sub(r"\s+", " ", m.group(0))),
    (r"CCID\s+1\.1", lambda m: "CCID 1.1"),
    (r"ICCD\s+1\.0", lambda m: "ICCD 1.0"),
    (r"USB\s+([23]\.\d)", lambda m: f"USB {m.group(1)}"),
    (r"HID\s+1\.11", lambda m: "HID 1.11"),
    (r"IAD\s+ECN", lambda m: "IAD ECN"),
    (r"PC/SC", lambda m: "PC/SC"),
]
# Abbreviations used inside one module only (piv-model writes P1 and P2 for SP 800-73-5 Parts 1 and 2).
MODULE_ALIASES: dict[str, list[tuple[str, str]]] = {
    "piv-model": [(r"\bP([12])\b", "SP 800-73-5 Part {0}")],
}

# References to the repository's own notes, not to a standard. A clause number after one is not placed.
INTERNAL = (
    r"\bdesign\b|\bnotes\b|\bROADMAP\b|\bPORTFOLIO\b|\bcrate (?:rule|choice)\b|\btask rule\b|"
    r"[\w/.\-]+\.(?:md|json)\b|\bcorpus/"
)


def iso_name(num: str, part: str | None, year: str | None) -> str:
    return "ISO/IEC " + num + (f"-{part}" if part else "") + (f":{year}" if year else "")


NUM = r"\d+(?:\.[\dx]+)*"
TABLE_NUMS = r"\d+(?:\.\d+)*(?:-\d+)?(?:(?:,\s*|\s+to\s+|\s+and\s+)\d+(?:\.\d+)*(?:-\d+)?)*"


def build_token_re(aliases: list[tuple[str, str]]) -> re.Pattern[str]:
    parts = []
    for i, (pat, _) in enumerate(DOC_PATTERNS):
        parts.append(f"(?P<doc{i}>{pat})")
    for i, (pat, _) in enumerate(aliases):
        parts.append(f"(?P<alias{i}>{pat})")
    parts.append(f"(?P<internal>{INTERNAL})")
    parts.append(rf"(?P<cl>§\s*(?P<n>{NUM})(?:[-–](?P<r>\d+(?:\.\d+)*))?)")
    parts.append(r"(?P<app>(?:Appendix|App\.|Annex)\s+(?P<a>[A-Z](?:\.\d+)*))")
    parts.append(r"(?P<bare>(?<![\w.])[A-F]\.\d+(?:\.\d+)*)")
    parts.append(rf"(?P<tab>(?P<tk>Tables?|Figures?)\s+(?P<t>{TABLE_NUMS}))")
    parts.append(r"(?P<q>¶\s*\d+(?:\s*[-–]\s*¶?\s*\d+)?|NOTE\s*\d*|Erratum\s+\d+|(?<=[\s\d])[a-z]\))")
    return re.compile("|".join(parts))


def sort_key(label: str) -> tuple:
    if label.startswith("§"):
        kind = 0
    elif label.startswith(("App.", "Annex")):
        kind = 1
    else:
        kind = 2
    nums = tuple(999 if p == "x" else int(p) for p in re.findall(r"\d+|x", label.replace("App. ", "").replace("Annex ", "")))
    return (kind, nums, label)


class Anchor:
    """The result of reading one Anchor cell."""

    def __init__(self) -> None:
        self.refs: dict[tuple[str, str], dict] = {}  # (doc, base) -> {"quals": [...], "inferred": bool}
        self.docs_seen: list[str] = []
        self.orphans: list[str] = []  # clauses that follow a sentence end with no document named
        self.internal = False


NOTES_PAREN = re.compile(r"\([^()]*notes[^()]*\)")
SENTENCE_END = re.compile(r"[.!?]\s+[A-Z]")


def parse_anchor(text: str, module: str, default_doc: str | None) -> Anchor:
    aliases = MODULE_ALIASES.get(module, [])
    tok = build_token_re(aliases)
    # Line ranges such as "(lines 2790-2792)" or "(3065-3131)" are not clauses.
    text = re.sub(r"\((?:lines?\s+)?[\d,\s\-–]+\)", " ", text)
    text = re.sub(r"\([^()]*from line \d+[^()]*\)", " ", text)
    text = re.sub(r'"[^"]*"', " ", text)  # quoted words are not citations
    text = NOTES_PAREN.sub(" ", text)  # "(§6 item 4 of the notes)" cites the notes
    # A backtick span is a path (kept as an internal marker) or code (dropped).
    text = re.sub(r"`([^`]*)`", lambda m: " design.md " if re.fullmatch(INTERNAL, m.group(1).strip()) else " ", text)
    res = Anchor()
    cur: str | None = None  # the document named most recently; None before one is named
    last: tuple[str, str] | None = None  # the clause a qualifier may attach to
    last_end = 0

    def put(doc: str, base: str, inferred: bool) -> None:
        slot = res.refs.setdefault((doc, base), {"quals": [], "inferred": inferred})
        slot["inferred"] = slot["inferred"] and inferred

    prev_ok = False  # the previous token was a document or a clause, so a bare table may follow it
    for m in tok.finditer(text):
        outer = m.lastgroup  # the outermost named group closes last
        assert outer is not None
        gap = text[last_end : m.start()]
        adjacent = gap.strip() == ""
        if SENTENCE_END.search(gap):
            # a new sentence: the document named before it no longer carries over
            cur, last, prev_ok = (None if cur != "@internal" else cur), None, False
        # A table with no clause before it is placed only when it directly follows a document or a list item
        # ("P1 Table 3", "Tables 2, 3", "§5.1 and Table 5"); a "Table 7" met in running prose is not trusted.
        tab_ok = prev_ok and re.fullmatch(r"[\s(,;]*(?:(?:and|or|to)\s+)?", gap) is not None
        prev_ok = outer != "internal"
        last_end = m.end()
        if outer.startswith("doc") or outer.startswith("alias"):
            if outer.startswith("doc"):
                i = int(outer[3:])
                cur = DOC_PATTERNS[i][1](re.fullmatch(DOC_PATTERNS[i][0], m.group(0)))
            else:
                i = int(outer[5:])
                cur = aliases[i][1].format(*re.search(aliases[i][0], m.group(0)).groups())
            res.docs_seen.append(cur)
            last = None
            continue
        if outer == "internal":
            res.internal = True
            cur, last = "@internal", None
            continue
        if outer == "q":
            if last is not None and adjacent:
                res.refs[last]["quals"].append(re.sub(r"\s+", " ", m.group("q")).strip())
            continue
        # a clause, an appendix, a table
        if cur == "@internal":
            last = None
            continue
        doc, inferred = cur, False
        if doc is None and default_doc is not None:
            doc, inferred = default_doc, True
        if outer == "cl":
            base = f"§{m.group('n')}"
        elif outer == "app":
            word = m.group("app").split()[0]
            base = f"{'Annex' if word == 'Annex' else 'App.'} {m.group('a')}"
        elif outer == "bare":
            base = f"App. {m.group('bare')}"
        else:
            base = m.group("tab").strip()
        if doc is None or (outer == "tab" and not tab_ok and not (last is not None and adjacent and last[0] == doc)):
            res.orphans.append(base)
            last = None
            continue
        if outer == "tab" and last is not None and adjacent and last[0] == doc:
            # a table named right after a clause qualifies it
            res.refs[last]["quals"].append(base)
            continue
        put(doc, base, inferred)
        last = (doc, base)
        r = m.group("r") if outer == "cl" else None
        if r:
            if "." in r:
                put(doc, f"§{r}", inferred)
                last = (doc, f"§{r}")
            else:
                res.refs[(doc, base)]["quals"].append(f"-{r}")
    return res


# ---------------------------------------------------------------------------------------------------------
# Test names
# ---------------------------------------------------------------------------------------------------------

TICKED = re.compile(r"`([^`]+)`")
PATHY = re.compile(r"[A-Za-z_][\w]*(?:(?:::|/|\.)[\w]+)*")
COLON_PATH = re.compile(r"[\w/.\-]*\w::\w+(?:::\w+)*")


def test_names(cell: str) -> list[str]:
    c = cell.strip()
    if re.match(r"(?i)(none|n/a|-)\b", c):
        return []
    names: list[str] = []
    for m in TICKED.finditer(c):
        t = m.group(1).strip()
        if PATHY.fullmatch(t) and ("_" in t or "::" in t or t.endswith(".rs")) and len(t) > 3:
            names.append(t)
    rest = TICKED.sub(" ", c)
    rest = re.sub(r"\[[^\]]*\]", " ", rest)  # mutation ids
    for m in COLON_PATH.finditer(rest):
        names.append(m.group(0))
    seen: list[str] = []
    for n in names:
        if n not in seen:
            seen.append(n)
    return seen


# ---------------------------------------------------------------------------------------------------------
# Reading the ROADMAP files
# ---------------------------------------------------------------------------------------------------------


def split_row(line: str) -> list[str]:
    """Split a table row on `|`, not inside a backtick span and not when escaped."""
    line = line.strip()
    inner = line[1:-1] if line.endswith("|") else line[1:]
    cells: list[str] = []
    cur: list[str] = []
    in_code = False
    i = 0
    while i < len(inner):
        ch = inner[i]
        if ch == "\\" and inner[i + 1 : i + 2] == "|":
            cur.append("|")
            i += 2
            continue
        if ch == "`":
            in_code = not in_code
        if ch == "|" and not in_code:
            cells.append("".join(cur).strip())
            cur = []
        else:
            cur.append(ch)
        i += 1
    cells.append("".join(cur).strip())
    return cells


class Row:
    def __init__(self, module: str, status: str, obligation: str, anchor: str, test: str, source: str) -> None:
        self.module, self.status, self.obligation, self.anchor, self.test, self.source = (
            module,
            status,
            obligation,
            anchor,
            test,
            source,
        )
        self.anchor_text = anchor
        self.tests = test_names(test)
        self.parsed: Anchor | None = None


def roadmap_files() -> list[tuple[str, Path]]:
    files: list[tuple[str, Path]] = []
    root = ROOT / "ROADMAP.md"
    if root.exists():
        files.append((ROOT_MODULE, root))
    for p in sorted(ROOT.glob("crates/*/roadmap/*.md")):
        files.append((p.stem, p))
    for p in sorted(ROOT.glob("crates/*/ROADMAP.md")):
        files.append((p.parent.name, p))
    return files


def read_tables(module: str, path: Path, problems: list[str]) -> tuple[list[Row], int]:
    rows: list[Row] = []
    rel = path.relative_to(ROOT).as_posix()
    lines = path.read_text(encoding="utf-8").splitlines()
    i = 0
    policy_rows = 0
    while i < len(lines):
        head = lines[i]
        if re.match(r"\|\s*Status\s*\|", head):
            cols = [c.lower() for c in split_row(head)]
            is_policy = cols[:3] == ["status", "policy", "why"]
            is_amb = cols == ["status", "ambiguity", "what the code does", "row or test"]
            if cols not in (["status", "obligation", "anchor", "test"], ["status", "policy", "why"]) and not is_amb:
                problems.append(f"{rel}:{i + 1}: table header not understood: {head}")
                i += 1
                continue
            mod = "(policy decisions)" if is_policy else module
            i += 2  # header and separator
            while i < len(lines) and lines[i].lstrip().startswith("|"):
                cells = split_row(lines[i])
                m = re.fullmatch(r"\[(ok|unread|open)\]", cells[0]) if cells else None
                if not m or len(cells) != len(cols):
                    problems.append(f"{rel}:{i + 1}: row not understood ({len(cells)} cells): {lines[i][:80]}")
                else:
                    if is_policy:
                        rows.append(Row(mod, m.group(1), cells[1], cells[2], "", rel))
                        policy_rows += 1
                    elif is_amb:
                        # no Anchor cell: the ambiguity and what the code does are read for citations
                        row = Row(mod, m.group(1), f"{cells[1]} - {cells[2]}", "", cells[3], rel)
                        row.anchor_text = f"{cells[1]} {cells[2]}"
                        rows.append(row)
                    else:
                        rows.append(Row(mod, m.group(1), cells[1], cells[2], cells[3], rel))
                i += 1
            continue
        i += 1
    return rows, policy_rows


def read_declared(problems: list[str]) -> dict[str, dict[str, int]]:
    """The per-module counts the root ROADMAP states in its own summary table."""
    declared: dict[str, dict[str, int]] = {}
    root = ROOT / "ROADMAP.md"
    if not root.exists():
        return declared
    lines = root.read_text(encoding="utf-8").splitlines()
    for idx, line in enumerate(lines):
        if re.match(r"\|\s*(Module|Crate)\s*\|", line) and "`[ok]`" in line:
            cols = [c.lower() for c in split_row(line)]
            j = idx + 2
            while j < len(lines) and lines[j].lstrip().startswith("|"):
                cells = split_row(lines[j])
                try:
                    ref = re.search(r"`([^`]+\.md)`", cells[cols.index("roadmap")])
                    assert ref
                    p = Path(ref.group(1))
                    name = p.parent.parent.name if False else (p.parent.name if p.name == "ROADMAP.md" else p.stem)
                    if p.name == "ROADMAP.md" and len(p.parts) == 1:
                        name = ROOT_MODULE
                    declared[name] = {
                        "ok": int(cells[cols.index("`[ok]`")]),
                        "unread": int(cells[cols.index("`[unread]`")]),
                        "open": int(cells[cols.index("`[open]`")]),
                        "tests": int(cells[cols.index("tests")]),
                    }
                except (ValueError, AssertionError, IndexError):
                    problems.append(f"ROADMAP.md:{j + 1}: summary row not understood: {lines[j][:80]}")
                j += 1
    return declared


# ---------------------------------------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------------------------------------


def esc(s: str) -> str:
    return s.replace("|", "\\|").replace("\n", " ")


def unanchored_reason(row: Row) -> str:
    a = row.parsed
    assert a is not None
    named = [d for d in a.docs_seen if not d.startswith("@")]
    if named:
        return "standard named, no clause: " + ", ".join(dict.fromkeys(named))
    if a.orphans:
        return "clause with no document named: " + ", ".join(dict.fromkeys(a.orphans))
    if a.internal:
        return "refers to the repository's own notes"
    if re.match(r"(?i)\s*(none|n/a|nothing)\b", row.anchor_text):
        return "no anchor"
    return "no recognisable clause"


def build() -> tuple[str, dict]:
    problems: list[str] = []
    all_rows: list[Row] = []
    policy_rows = 0
    for module, path in roadmap_files():
        rows, p = read_tables(module, path, problems)
        all_rows.extend(rows)
        policy_rows += p
    declared = read_declared(problems)
    if problems:
        for p in problems:
            print("matrix: " + p, file=sys.stderr)
        raise SystemExit(2)

    for r in all_rows:
        r.parsed = parse_anchor(r.anchor_text, r.module, DEFAULT_DOC)

    placed = [r for r in all_rows if r.parsed and r.parsed.refs]
    unplaced = [r for r in all_rows if not (r.parsed and r.parsed.refs)]

    # summary by module
    order: list[str] = []
    for r in all_rows:
        if r.module not in order:
            order.append(r.module)
    order.sort(key=lambda m: (m in (ROOT_MODULE, "(policy decisions)"), m == "(policy decisions)"))
    out: list[str] = []
    w = out.append
    w("# Reference matrix")
    w("")
    w(
        "<!-- Generated by `python tools/matrix.py` from the ROADMAP tables; do not edit by hand. "
        "`python tools/matrix.py --check` fails when this file is stale. -->"
    )
    w("")
    w(
        "This matrix is rebuilt from the `Status | Obligation | Anchor | Test` tables of `ROADMAP.md` and the module "
        "fragments under `crates/*/roadmap/`. Nothing in it is written by hand: each cell is copied from a ROADMAP "
        "row, and the only computed things are the counts and the split of an Anchor cell into a document and "
        "clauses. A row that cites two clauses appears under both. Status words are the ROADMAP's: `[ok]` the "
        "named test exists, passes, and failed when the code was deliberately broken; `[unread]` the test passes "
        "but the anchor could not be read cleanly; `[open]` not done."
    )
    w("")
    w(f"Rows read: {len(all_rows)}. Placed under a clause: {len(placed)}. Not placed (listed under Unanchored): "
      f"{len(unplaced)}.")
    w("")
    w("## Summary by module")
    w("")
    w("| Module | `[ok]` | `[unread]` | `[open]` | Rows | Test names cited | Rows with a clause | Unanchored |")
    w("|---|---:|---:|---:|---:|---:|---:|---:|")
    tot = {"ok": 0, "unread": 0, "open": 0, "rows": 0, "placed": 0, "un": 0}
    all_tests: set[tuple[str, str]] = set()
    per_mod: dict[str, dict] = {}
    for m in order:
        rs = [r for r in all_rows if r.module == m]
        c = {s: sum(1 for r in rs if r.status == s) for s in STATUSES}
        tests = {t for r in rs for t in r.tests}
        pl = sum(1 for r in rs if r.parsed and r.parsed.refs)
        per_mod[m] = {**c, "tests": len(tests), "rows": len(rs)}
        all_tests |= {(m, t) for t in tests}
        w(f"| {esc(m)} | {c['ok']} | {c['unread']} | {c['open']} | {len(rs)} | {len(tests)} | {pl} | {len(rs) - pl} |")
        for s in STATUSES:
            tot[s] += c[s]
        tot["rows"] += len(rs)
        tot["placed"] += pl
        tot["un"] += len(rs) - pl
    w(
        f"| **Total** | {tot['ok']} | {tot['unread']} | {tot['open']} | {tot['rows']} | {len(all_tests)} | "
        f"{tot['placed']} | {tot['un']} |"
    )
    w("")
    w(
        "`Test names cited` counts the distinct test names that appear in the Test cells of the module's rows. "
        "It is not a count of tests in the code: a test no row names is not counted, and a name in a Test cell "
        "that no longer exists in the code is."
    )
    if policy_rows:
        w("")
        w(
            f"`(policy decisions)` is the `Status | Policy | Why` table of `ROADMAP.md`: {policy_rows} rows, each "
            "a choice where the standard is silent. Its Why cell is read as the anchor and it has no Test cell."
        )
    w("")
    w("### Against the counts the ROADMAP states")
    w("")
    w(
        "The root ROADMAP keeps its own per-module counts by hand. Where they differ from the rows, the rows are "
        "what this file uses."
    )
    w("")
    w("| Module | `[ok]` stated / rows | `[unread]` stated / rows | `[open]` stated / rows | Tests stated / names cited |")
    w("|---|---:|---:|---:|---:|")
    for m in order:
        d = declared.get(m)
        if not d:
            continue
        c = per_mod[m]

        def cell(k: str, mine: int) -> str:
            return f"{d[k]} / {mine}" + ("" if d[k] == mine else " **differs**")

        w(f"| {esc(m)} | {cell('ok', c['ok'])} | {cell('unread', c['unread'])} | {cell('open', c['open'])} | "
          f"{cell('tests', c['tests'])} |")
    w("")

    # by document
    doc_rows: dict[str, dict[tuple[str, str], list[tuple[Row, list[str], bool]]]] = {}
    for r in placed:
        assert r.parsed
        for (doc, base), info in r.parsed.refs.items():
            doc_rows.setdefault(doc, {}).setdefault((base, ""), []).append((r, info["quals"], info["inferred"]))
    w("## Summary by standard document")
    w("")
    w("| Document | Clauses cited | Rows | `[ok]` | `[unread]` | `[open]` |")
    w("|---|---:|---:|---:|---:|---:|")
    for doc in sorted(doc_rows):
        ents = doc_rows[doc]
        rowset = {id(r): r for lst in ents.values() for r, _, _ in lst}
        c = {s: sum(1 for r in rowset.values() if r.status == s) for s in STATUSES}
        w(f"| {esc(doc)} | {len(ents)} | {len(rowset)} | {c['ok']} | {c['unread']} | {c['open']} |")
    w("")
    w(
        "A row that cites several clauses of one document counts once for that document and once per document it "
        "cites, so the Rows column does not add up to the total."
    )
    w("")
    inferred_n = sum(1 for r in placed for info in r.parsed.refs.values() if info["inferred"])  # type: ignore[union-attr]
    w("## Reference matrix")
    w("")
    intro = (
        "Grouped by standard document, then clause. The clause cell shows what follows the clause number in the "
        "Anchor cell (paragraph, NOTE, item letter, table) as written."
    )
    if DEFAULT_DOC is not None:
        intro += (
            f" A clause marked † names no document in its own cell; it is read as {DEFAULT_DOC} "
            f"({inferred_n} clause references)."
        )
    w(intro)
    for doc in sorted(doc_rows):
        w("")
        w(f"### {doc}")
        w("")
        w("| Clause | What the code does | Module | Status | Test |")
        w("|---|---|---|---|---|")
        keys = sorted(doc_rows[doc], key=lambda k: sort_key(k[0]))
        for key in keys:
            for r, quals, inf in doc_rows[doc][key]:
                clause = key[0] + ("".join(" " + q for q in dict.fromkeys(quals))) + (" †" if inf else "")
                w(f"| {esc(clause)} | {esc(r.obligation)} | {esc(r.module)} | `[{r.status}]` | {esc(r.test)} |")
    w("")
    w("## Unanchored")
    w("")
    w(
        f"{len(unplaced)} rows have no recognisable clause in their Anchor cell. They are listed in full with the "
        "Anchor as written and the reason."
    )
    w("")
    w("| Module | Status | What the code does | Anchor as written | Test | Reason |")
    w("|---|---|---|---|---|---|")
    for r in unplaced:
        assert r.parsed
        w(
            f"| {esc(r.module)} | `[{r.status}]` | {esc(r.obligation)} | {esc(r.anchor or "(no Anchor cell in this table)")} | {esc(r.test)} | "
            f"{esc(unanchored_reason(r))} |"
        )
    orphaned = [r for r in placed if r.parsed and r.parsed.orphans]
    if orphaned:
        w("")
        w("## Clauses with no document")
        w("")
        w(
            "These rows are placed above, but their Anchor cell also names a clause that follows the end of a "
            "sentence with no document of its own, so the generator did not guess which standard it belongs to."
        )
        w("")
        w("| Module | What the code does | Anchor as written | Clause left out |")
        w("|---|---|---|---|")
        for r in orphaned:
            assert r.parsed
            w(f"| {esc(r.module)} | {esc(r.obligation)} | {esc(r.anchor)} | {esc(', '.join(r.parsed.orphans))} |")
    stats = {
        "rows": len(all_rows),
        "placed": len(placed),
        "unplaced": len(unplaced),
        "docs": len(doc_rows),
        "tot": tot,
        "tests": len(all_tests),
        "inferred": inferred_n,
    }
    return "\n".join(out), stats


def main(argv: list[str]) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    check = "--check" in argv
    text, st = build()
    print(
        f"matrix: {st['rows']} rows read, {st['placed']} placed under {st['docs']} documents, "
        f"{st['unplaced']} could not be placed (listed under Unanchored); "
        f"ok {st['tot']['ok']}, unread {st['tot']['unread']}, open {st['tot']['open']}"
    )
    if check:
        have = OUT.read_text(encoding="utf-8").replace("\r\n", "\n") if OUT.exists() else None
        if have != text:
            print("matrix: docs/matrix.md is stale; run `python tools/matrix.py`", file=sys.stderr)
            return 1
        print("matrix: docs/matrix.md is current")
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(f"matrix: wrote {OUT.relative_to(ROOT).as_posix()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
