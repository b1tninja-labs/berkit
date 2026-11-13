# Changes to the corpus format and to the corpus

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
