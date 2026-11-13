# Changes to the corpus format and to the corpus

## corpus 0.2.0, format revision 1.2

Why: the corpus could state a decode outcome only as a TLV tree, a hex string or an error class, so it could not state
the VALUE a typed decode produces. Proof of the gap: breaking the OID first-arc split on decode in the Rust crate
(`v - 40` to `v - 41`) failed five module tests and no corpus case, and any implementation could decode OIDs, INTEGERs
or times wrongly and pass. This revision closes it.

1. **New outcome `value`** (decode and roundtrip). It stands alone or beside `tree`; never with `hex` or `error`. A tree-only
   outcome means what it meant. Changed: `format.md` ("Decoded values", "Outcomes"), `case.schema.json` (`outcome`).
2. **Value forms for the types that had none**: `ENUMERATED` (decimal string), `UTCTime` and `GeneralizedTime` (a record of
   year, month, day, hour, minute, second, fraction digits, comma, zone), `REAL` (an opaque record of the first-octet class and
   the contents hex). The existing `typed.value` forms are reused for the decoded value. Types with no form stay tree-only,
   and the format says which. Changed: `format.md`.
3. **New optional case field `pivot`** (0 to 99), for `UTCTime` cases that state a value; it names the century pivot that the
   `utctime-pivot` policy leaves to the caller. Changed: `case.schema.json`, `format.md`, `tools/check.py`.
4. **New case file `cases/values.json`**: 248 decode and roundtrip cases with a `value`, every expectation constructed by
   hand from the clause (OBJECT IDENTIFIER first-arc split and arcs of 2^32, 2^64 and 128 bits, RELATIVE-OID, INTEGER and
   ENUMERATED across every byte-width boundary and beyond 64 bits, BOOLEAN, NULL, OCTET STRING, BIT STRING, character
   strings, UTCTime, GeneralizedTime, REAL). Ids start `x690.<clause>.val-`.
5. **Tools**: `tools/refdec.py` gains `decode_value`, an independent reading of the clauses, and `tools/check.py` runs it
   over every value outcome and checks the rules above.

It is a MINOR bump because it adds a new optional outcome form, a new optional field and new cases, and changes no
existing case, id or expectation; a 0.1.0 runner that skips outcomes it does not know still passes every old case.
A runner that wants the new guarantee must check `value`. The manifest now lists six files.

## corpus 0.1.0, format revision 1.1

The first version. It starts from `format.md` and `case.schema.json` as received (format 1.0) and makes these
changes. All are additive for a runner that ignores what it does not know, but they are recorded as a MINOR bump of
the format (1.0 to 1.1) because a runner must know the new class and may use the new field.

1. **New error class `stray_eoc`** (24 classes now; 13 shared, 11 DER-only). Added at the coordinator's request after
   berkit main added it (`ErrorClass::StrayEoc`): end-of-contents octets, or the UNIVERSAL 0 identifier with more
   than two EOC octets, where no indefinite-length encoding is open. Refused in BER and DER, so it is a shared
   class and never appears under `der` alone. Clauses: X.690 §8.1.5 clause 1, §8.1.1.2; X.680 §8.4 Table 1.
   Changed: `format.md` (class table), `case.schema.json` (enum), `corpus.json` (`error_classes`), `tools/check.py`
   (count 24, shared versus DER-only check).
2. **New optional case field `policy`.** A key of the `policies` table of `corpus.json` (the ten ROADMAP policy rows).
   It marks a case whose outcome rests on a policy decision where the standard is silent. Changed: `case.schema.json`,
   `format.md`.
3. **New type prefix `corpus:<Name>`** for the five small schema types defined in `types.md`. Changed: `format.md`.
4. **Manifest.** `corpus.json` follows `format.md` ("corpus_version", "corpus_format", per-file sha256, `limits`,
   editions) and adds `error_classes`, `der_only_error_classes`, `policies`, `schema` and `corpus_format_revision`.
   The two limits above 2^53 are decimal strings. The brief called the version field `version`; the format's name,
   `corpus_version`, is used.
5. **Files hold the wrapper object** `{"corpus_format": "1", "cases": [...]}` of the schema, not a bare array.

Version rules for the corpus itself are in `README.md`.
