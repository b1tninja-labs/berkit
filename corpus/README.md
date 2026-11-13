# berkit conformance corpus

Language-neutral test cases for ASN.1 BER and DER, built from ITU-T X.690 (02/2021) and X.680 (02/2021) and the notes in
`docs/design/`. Format: `format.md` (revision 1.2) and `case.schema.json`. Manifest: `corpus.json`. Hand-made schema
types: `types.md`. Notes that disagreed or were wrong: `CORRECTIONS.md`. Format changes: `CHANGES.md`.

## Layout

    corpus.json              manifest: corpus_version, per-file sha256, the 24 error classes, limits, policy keys
    cases/framing.json       identifier, length, EOC, nesting, truncation, limits, header encode cases
    cases/primitives.json    BOOLEAN, INTEGER, ENUMERATED, NULL, BIT/OCTET STRING, OID, RELATIVE-OID, REAL, OID-IRI, typed encodes
    cases/structured.json    SEQUENCE, SET, SET OF order, tagging, DEFAULT, INSTANCE OF
    cases/strings-time.json  character strings (alphabets exhaustive for Printable and Numeric), UTCTime, GeneralizedTime, TIME family
    cases/values.json        decoded typed values (format revision 1.2): OID arcs, INTEGER, times, strings, BIT STRING, REAL, ...
    cases/annex.json         examples printed in X.690 clauses and annexes (A.3 record, Annex B, C.5)
    tools/check.py           validator and self-check (stdlib only); tools/refdec.py is its reference decoder and encoder

`sample-cases.json` is the illustrative sample that came with the format. Its ids reappear in `cases/` (same input and
outcome except as noted in CORRECTIONS.md); it is not part of the manifest.

## Versioning

`corpus_version` is semver. Adding cases is a minor bump. Changing a case's expectation, input or type, deleting a
case, or changing the format is a major bump. (A change that only adds an optional field or a class to the format is
recorded in `CHANGES.md` as a format revision and is a minor bump of the corpus.) Ids are never reused: a retracted case
keeps its id in `cases/retired.json` with a reason (no such file yet). Each file's sha256 is in `corpus.json`;
`python corpus/tools/check.py --write-manifest` recomputes them after an edit, and every edit needs a version bump.

## How another language depends on it

By relative path inside the workspace (for example `../berkit/corpus` from a sibling project) or as a git submodule pinned
to a tag. A test harness reads `corpus.json`, refuses to run when `corpus_format` or the major of `corpus_version` differs
from the one it was written for, checks each file's sha256, then runs each case in each mode it names, taking `limits` from
the manifest. Tag numbers (up to 2^64-1) and `limits.max_length` exceed 2^53: read them as u64 or big integers. No generated
copy of the corpus is checked into an implementation.

## Conventions worth knowing

- Decoded values (revision 1.2): an outcome of a typed decode or roundtrip may name `value`, the decoded value, so a
  case pins what the type MEANS and not only the TLV it is made of. `cases/values.json` holds 248 such cases, every
  expected value constructed by hand from the clause (the provenance shows the arithmetic). A runner must check the
  value for every case that states one. Types with no value form (SEQUENCE, SET, DATE, TIME-OF-DAY, DATE-TIME, DURATION,
  TIME, OID-IRI, RELATIVE-OID-IRI and the unchecked 8-bit strings) stay tree-only. `UTCTime` values carry the case's
  `pivot`. Forms and rules: `format.md`.

- Expectation keys: `both`, or `ber` and/or `der`. "BER accepts, DER rejects" is one case. A case that names one mode
  says nothing about the other.
- Stream order picks the error when there are several faults (format.md). Ties settled here: a lone final `00` inside an
  open indefinite-length encoding is the identifier of an element with no length octet (`truncated`, or `length_overrun`
  inside a definite-length ancestor); a universal tag 0 element that is not the closing EOC is `stray_eoc`; inside a
  definite-length parent every child shortfall is `length_overrun`; a constructed string in DER is `constructed_string`
  (decided at the identifier) before any length fault.
- `policy` marks cases that rest on a ROADMAP policy row; the keys and rows are in `corpus.json`. `ber-shall-rules` is used
  where the rejection is derived (alphabets, character codings, time ranges, empty OID), `limits` for the manifest limits.
  Rejections that follow directly from a clause of 8 to 11 carry no policy.
- Provenance is the notes' own: `from X.690 <place>` only where the standard prints the bytes or the value; otherwise
  `constructed by hand from <clause>`.
- Left out because the notes mark the outcome as open or the format cannot state it: the function-level `compare(a, b)`
  vectors of SET OF padding (not TLVs); the CER ordering vector (no CER mode); tag-mismatch and tagged-CHOICE vectors (no
  class for a schema tag mismatch); UTF-8 surrogates, values above U+10FFFF and truncated sequences (ISO/IEC 10646 Annex D not
  read); BMPString U+FFFE and U+FFFF; UniversalString surrogates; GeneralizedTime seconds 60 and a decimal sign without digits
  (ISO 8601 not read); `1F 23 00`; REAL with a one-octet form-11 exponent; the DER form of a decimal REAL (ISO 6093 not read);
  the full TIME grammar beyond the vectors of strings-time.md.
