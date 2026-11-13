# Conformance corpus format, `corpus_format` 1

Plain JSON, UTF-8, no BOM. One file holds `{"corpus_format": "1", "cases": [ ... ]}`. Validate with
`case.schema.json` (JSON Schema 2020-12). Sample: `sample-cases.json` (28 cases; all hex checked
against the standard's printed bytes or built by hand and labelled so). I could only check the schema's
patterns with a hand script, not run a full JSON Schema validator (none installed here); run
`jsonschema` or `ajv` before relying on it.

## Case

| Field | Meaning |
|---|---|
| `id` | Stable, unique, never reused: `<source>.<clause>.<slug>`, e.g. `x690.8.19.oid-2-999-3`. |
| `title` | Free text. |
| `op` | `decode`, `encode`, or `roundtrip`. |
| `type` | `null` for an untyped structural decode. Else a universal type name (`BOOLEAN`, `OBJECT IDENTIFIER`, `GeneralizedTime`, ...) or `<doc>:<clause>:<Name>` for a type the standard defines, e.g. `x690:A.1:PersonnelRecord`. Type definitions are not copied into the corpus (they are the standard's text); the implementation of a typed runner writes them from the cited clause. |
| `input` | Exactly one of `hex` (lowercase, even length, no spaces), `tree` (a node, below), `typed` (`{type, value}`, value in a JSON form defined per type below). |
| `expect` | Per-mode outcomes, below. |
| `clause` | Array of citations, each `X.690 (02/2021) §8.19.4`-style. The first is the clause that decides the main outcome. |
| `provenance` | `from X.690 <place>`, `from X.680 <place>`, or `constructed by hand from <clause>`. Hand-built cases must be so labelled; they are the least authoritative. |
| `edition` | `X.690:2021-02`, or `X.690:2021-02+E1` when Erratum 1 (09/2021) is involved, or the `X.680:` equivalent. |

### Operations

- `decode`: `input.hex` is parsed in each mode named in `expect`. Outcome is a tree or an error class.
  With `type` null only structural rules (identifier, length, EOC, nesting, trailing data) apply.
- `encode`: `input.tree` or `input.typed` is serialised; `expect.<mode>.hex` is the exact output.
  The tree form tests the writer's header handling (it must reproduce `length.form` and `octets`);
  the typed form tests content (OID packing, INTEGER minimality). A BER writer has options (§8.1.3.2,
  §7.3), so an `encode` case with a `ber` entry holds only where the standard leaves one choice, or
  where the case states the writer policy; prefer asserting `encode` under `der` or `both`. In every
  case, a runner must also check that its output decodes back to the input tree.
- `roundtrip`: `decode`, then `encode` of the decoded tree. In `der`, the re-encoded bytes must equal
  `input.hex`. In `ber`, bytes need not match (alternative forms are a sender's option, §7.3), but
  decoding the re-encoded bytes must give a tree equal to `expect` in all fields except `length`
  (and `children` order must be unchanged; SET/SET OF order is not significant in BER, but a decoder
  that reorders would fail this and that is intended).

### Tree

Node: `{"class": "universal|application|context|private", "constructed": bool, "tag": n,
"length": {"form": "short|long|indefinite", "octets": k (long only)}, "children": [...] | "content": hex,
"asn1": "annotation, ignored"}`. `constructed` true requires `children` (empty list allowed), false
requires `content` (empty string for zero contents). For `indefinite`, the EOC is implied, not a child.
`length.octets` is the number of length octets after the initial one, so a non-minimal long form such
as `81 01` is `{"form":"long","octets":1}` with 1 content octet. Only decodable encodings appear in
trees, so non-minimal tags never occur there. Tag numbers 31 and above are written as the number.
Class and bit 6 are in `class` and `constructed` (§8.1.2.2, §8.1.2.5).

### `typed.value` JSON forms

`BOOLEAN`: true/false. `INTEGER`: decimal string. `NULL`: null. `OBJECT IDENTIFIER` and
`RELATIVE-OID`: dotted string. Character strings: the string. `OCTET STRING`: hex. `BIT STRING`:
`{"unused": n, "hex": ...}`. Others added by version bump only.

## "BER accepts, DER rejects" without duplication

`expect` has keys `both`, `ber`, `der`. Either `both` alone, or `ber` and/or `der` (never `both` with
either). A case runs once per mode it names. Example: `{"ber": {"tree": ...}, "der": {"error":
"boolean_not_ff"}}`. A case that names only `der` says nothing about BER (used where I have not
established what BER does, e.g. the DER time-format cases). The input is shared, so there is nothing to
duplicate.

## Outcomes

An outcome has exactly one of `tree` (decode/roundtrip success), `hex` (encode success), `error`
(error class). `error_offset` (octet offset of first violation) is permitted only as an advisory field
and is not required to match.

Rule for which error wins when an input has several faults: **stream order**. Identifier octets, then
length octets, then contents, elements depth first in the order they appear; the first violation is
reported. In that order a header fault in an element is reported before a fault in its contents. For
`set_order`, the fault is detected at the identifier of the first component that is out of order.
`wrong_form` (constructed bit contradicting the type) is detected at the identifier.

## Error classes (fixed list)

Clauses are cited from the text of X.690 (02/2021) as read through `itu_read`/`pdftotext`.

Rejected in both BER and DER:

| Class | Definition | Clause |
|---|---|---|
| `truncated` | input ends inside the identifier, the length octets, or the declared number of contents octets | §8.1.1.1, §8.1.3.3 |
| `reserved_length` | initial length octet is `ff`, which the long form must not use | §8.1.3.5 c) |
| `indefinite_on_primitive` | primitive identifier (bit 6 = 0) with the indefinite length `80`; primitive encodings use the definite form | §8.1.3.2 a), §8.1.3.6 |
| `length_overrun` | a child's header or contents extend past the end of its enclosing definite-length contents | §8.1.3.3, §8.1.4 |
| `missing_eoc` | an indefinite-length encoding reaches the end of its enclosing definite-length contents (or the input, when top-level) without `00 00` | §8.1.3.6.2, §8.1.5 |
| `non_minimal_tag` | tag number 0-30 written in the high-tag-number form, or first subsequent identifier octet has bits 7-1 all zero | §8.1.2.2, §8.1.2.4.2 c) |
| `limit_exceeded` | a length, tag number, or nesting depth beyond the limit the implementation declares in the corpus manifest; no clause (the standard sets no bound: §8.1.3.5 allows up to 126 length octets) | none; §8.1.3.5 |
| `wrong_form` | constructed bit contradicts the type: e.g. constructed BOOLEAN, INTEGER, NULL; primitive SEQUENCE | §8.2.1, §8.3.1, §8.8.1, §8.9.1, §8.1.2.5 |
| `bad_contents` | contents violate the type's rule, e.g. BOOLEAN with length other than 1, NULL with contents, BIT STRING initial octet > 7 | §8.2.1, §8.8.2, §8.6.2.2 |
| `non_minimal_integer` | INTEGER (or ENUMERATED) contents longer than the smallest two's-complement form | §8.3.2 |
| `non_minimal_subidentifier` | OID or RELATIVE-OID subidentifier starting with octet `80` | §8.19.2, §8.20.2 |
| `trailing_data` | octets remain after the single top-level encoding. API-level: the standard describes one encoding; reporting leftover input is a design decision of this corpus | §8.1.1.1 (derived) |

Rejected in DER (accepted by BER unless noted):

| Class | Definition | Clause |
|---|---|---|
| `non_minimal_length` | definite length not in the fewest octets (long form where short fits, or leading zero length octet) | §10.1; BER allows: §8.1.3.5 NOTE 2 |
| `indefinite_length` | indefinite length used | §10.1 |
| `constructed_string` | constructed encoding of BIT STRING, OCTET STRING or a restricted character string | §10.2 |
| `boolean_not_ff` | BOOLEAN TRUE with a contents octet other than `ff` | §11.1 |
| `unused_bits_nonzero` | unused bits in the final octet of a BIT STRING not zero | §11.2.1 |
| `bitstring_trailing_zeros` | trailing 0 bits not removed where X.680 §22.7 applies (named-bit lists) | §11.2.2 |
| `set_order` | SET components not in canonical tag order | §10.3, X.680 §8.6 |
| `setof_order` | SET OF encodings not in ascending octet-string order (shorter padded with trailing 0 octets) | §11.6 |
| `default_encoded` | SET/SEQUENCE component equal to its DEFAULT is present | §11.5 |
| `time_format` | GeneralizedTime/UTCTime not in the §11.7/§11.8 form (no `Z`, seconds absent, trailing zero in fraction, wrong midnight) | §11.7, §11.8 |
| `real_form` | REAL not in the §11.3 form (base 2 with F = 0 and odd mantissa; NR3 decimal) | §11.3 |

The CER rules (§9) are not a mode in format 1; add `cer` as a mode and its classes by a version bump
if wanted (they are the same shape, with `indefinite_length` inverted for constructed forms).

Where a class needs a type (`wrong_form`, `bad_contents`, the DER content classes, `set_order`), the
case must carry `type`.

## Versioning and location

- Layout, one repository (or one directory) with its own history, e.g. `ber-corpus/`:
  `corpus.json` (manifest: `corpus_version` semver, `corpus_format`, per-file sha256, `limits`
  declared for `limit_exceeded`, the editions covered), `schema/case.schema.json`,
  `cases/x690-annex-a.json`, `cases/x690-clause8.json`, `cases/x690-clause10-11.json`,
  `cases/hand-*.json`. Split by provenance so that "derived from a printed example" and "constructed by
  hand" cases can be weighted differently.
- Version: semver on the corpus as a whole. Adding cases or a new error class = minor. Changing an
  existing case's expectation, deleting a case, or changing the format = major. Ids are immutable;
  a retracted case keeps its id in `cases/retired.json` with a reason, never reused.
- Two implementations (Rust crate, Python package) depend on it by relative path inside the same
  workspace, e.g. `../ber-corpus` (workspace siblings, as the other projects under `D:\code`), or
  a git submodule at `tests/corpus` pinned to a tag. Each test harness reads `corpus.json`, refuses to run
  if `corpus_version` major differs from the one it was written for, and checks each file's sha256 so
  a silent edit fails loudly. No generated copy of the corpus is checked into either implementation.
- Edition: each case's `edition` is the standard it was derived from; a new edition of the standard
  adds cases or bumps a case's id suffix (`.v2`) rather than mutating the old.
