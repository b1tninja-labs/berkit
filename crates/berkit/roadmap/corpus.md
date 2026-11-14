# Corpus runner roadmap (tests/corpus.rs, tests/common/, tests/corpus_known.md)

The runner reads `corpus/` (0.2.0, format revision 1.2, 1179 cases, 6 files) and runs every case, in every mode it names, against the crate.
The `itulib` index was unavailable in this session (`itu_read` returned "no such table: meta"), so no clause was
re-read from X.690; the only classification so far is a design question, not a clause question. A breakage `M n`
was restored from a copy kept outside the repository and verified with `diff` and `git status`; `cargo clean -p berkit`
ran between runs.

Update 2026-11-13: clause reading works through `python -m primarysource.cli itu read "X.690§8.1.3.5"` (run from `a sibling directory of the workspace` with `PYTHONUTF8=1`); the MCP `itulib` tools still fail. The framing clauses were re-read that way (see `framing.md`). The decoded-value cases below were not: their clauses are still unread.

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | All 1179 cases run, once per mode the case names: 2355 runs (decode 690 ber + 690 der, roundtrip 445 ber + 444 der, encode 44 ber + 42 der); 1063 cases are typed, 541 carry a `policy` | `corpus/format.md` Operations | `corpus_agrees_with_the_code_except_the_known_list` (prints the table with `--nocapture`); M1 `boolean.rs` DER check off (2 new), M2 Printable alphabet off (183 new disagreements), M3 `time.rs` DER zone check off (1 new), M4 `tlv.rs` DER indefinite check off (20 new) |
| [ok] | DECODED VALUES (format revision 1.2): an outcome may name `value`, alone or beside `tree`. 248 new cases in `cases/values.json` (201 roundtrip, 47 decode), every value constructed by hand from the clause: OBJECT IDENTIFIER 42, RELATIVE-OID 14, INTEGER 41, ENUMERATED 14, BOOLEAN 6, NULL 1, OCTET STRING 9, BIT STRING 16, UTF8String 15, BMPString 9, UniversalString 4, PrintableString 3, NumericString 2, IA5String 3, VisibleString 3, UTCTime 20, GeneralizedTime 28, REAL 18. The runner compares the value in the corpus forms: INTEGER and ENUMERATED as a decimal string of any size (base-256 to decimal in `common/typed.rs`, checked against `is_negative`, `write_magnitude`, `to_i64`, `to_u64`, `to_i128`, `to_u128`), OID and RELATIVE-OID arcs through `Arc::be_bytes` (checked against `to_u64`, `to_u128`), BIT STRING from `unused_bits` and `data` or `copy_data`, OCTET STRING, strings (the crate validates, `std` decodes), times from `Fields`, REAL from `Kind` and `contents`. A panic inside the crate is now a disagreement of that case, not the end of the run. 0 disagreements: the crate gives every stated value | `corpus/format.md` "Decoded values" | `corpus_agrees_with_the_code_except_the_known_list`; `a_wrong_value_is_reported`; `python corpus/tools/check.py` runs the independent Python reference decoder (`refdec.decode_value`) over all 449 value outcomes |
| [ok] | The value outcome catches what the tree outcome missed. Each break was made in `src/`, the edit printed and diffed, `cargo clean -p berkit`, `cargo test --test corpus`, the file restored from a copy outside the repository and checked with `diff`. New disagreements (all cases, of which in values.json): a) `oid.rs` first arc `v - 40` to `v - 41`: 23 (22; the one old case, `x690.8.19.oid-1-0`, only panics); b1) `integer.rs` `to_i64` width 8 to 7: 2 (2); b2) `integer.rs` `to_i128` negative fill `0xFF` to `0x00`: 30 (20); c) `bitstring.rs` `unused_bits` minus 1: 8 (8); d1) `time.rs` GeneralizedTime month capped at 11: 3 (3); d2) UTCTime hour capped at 22: 7 (7); e1) UTF-8 `F4` upper bound `8F` to `8E`: 1 (1); e2) BMPString limit `FFFE` to `FFEE`: 1 (1). A sweep of 20 further breaks (UTCTime month, day, minute, second, pivot boundary, zone sign and minute presence; GeneralizedTime hour, comma, fraction, century; OID zero-arc and bias; BER BOOLEAN; REAL minus zero; INTEGER magnitude; arc width; `to_u128` width; constructed OCTET STRING) was caught in each case. One break was NOT caught at first: constructed BIT STRING `copy_data` no longer masking the unused bits (h16). Two cases with pad bits set in the last segment (`x690.8.6.val-bitstring-constructed-padbits-4` and `-7`) close it (2 new disagreements) | task rule | `corpus_agrees_*` |
| [ok] | The set of disagreeing ids equals `tests/corpus_known.md` exactly: a new disagreement and a case that now agrees both fail; every entry has a classification and a reason | task rule | same test; `a_wrong_expectation_is_reported` shows the comparison can fail |
| [ok] | Each case file's sha256 equals `corpus.json`, files read as bytes, no line-ending change; major version 0 and format 1 required | `corpus/README.md` "How another language depends on it" | `corpus_files_match_the_manifest_sha256`; SHA-256 against the FIPS 180-4 vectors (empty, `abc`, the 448-bit message, one million `a`) and the 55 and 56 octet padding edges in `common/sha256.rs` |
| [ok] | Limits come from the manifest (depth 64, length and tag number 2^64-1, kept as decimal strings) | `corpus.json` `limits` | `corpus_agrees_*` asserts them; the 9 `limits` cases agree |
| [ok] | Tree outcome compares class, constructed bit, tag number, length form and octet count, children or content; error outcome compares the `ErrorClass` NAME; `error_offset` is not compared | `corpus/format.md` Tree, Outcomes | `corpus_agrees_*`; `a_wrong_expectation_is_reported` |
| [ok] | Stream order: the identifier decides `wrong_form` and DER `constructed_string` before any length fault; trailing data is reported last | `corpus/format.md` Outcomes | `typed::decode`; the constructed-string and `wrong_form` cases agree |
| [ok] | BER roundtrip decodes the re-encoded bytes and compares all fields but the length; DER re-encoding equals the input | `corpus/format.md` Operations | `run_one` |
| [ok] | An encode output decodes back to the input tree (typed: decodes without error) | `corpus/format.md` Operations | `run_one` |
| [ok] | JSON reader: escapes, surrogate pairs, numbers kept as text, malformed input refused, depth bounded | none | `common/json.rs` unit tests |
| [open] | `x690.8.1.3.encode-length-nonminimal-81-01`: the writer cannot write surplus length octets | X.690 §8.1.3.5 NOTE 2 | POLICY-GAP in `corpus_known.md`; decision needed: a writer option or retire the case |
| [open] | Everything below is done by the TEST ADAPTER, not by the crate; it may become a library module | `docs/design/structured.md` §5 | `common/typed.rs` |

## What the adapter does that the crate does not

The crate has no typed layer. `common/typed.rs` supplies: the form check from the identifier (`wrong_form`, DER
`constructed_string`); the tag-number check against a schema; flattening of constructed BIT STRING, OCTET STRING,
character strings and time strings into segments (segment tag 3 or 4, else `bad_contents`); DER SET order by
class then tag number (`set_order`); DER SET OF order by padded encodings (`setof_order`); DEFAULT INTEGER components
(`default_encoded`); explicit and implicit tagging; the schemas of `corpus/types.md`, X.690 §8.14 Type1 to Type5 and the
Annex A.1 PersonnelRecord (from the paraphrase in `docs/design/annex-examples.md`, not from the clause); and tree-to-bytes
serialisation with `tlv::write_header`, `write_eoc`. The `set-vs-setof` policy is met by naming the type: `SET` applies the
tag order, `SET OF` the encoding order.

## What is not run, and why

- `error_offset` is advisory and never compared (format.md).
- Components of an inline `SEQUENCE {...}` type are not checked: no case depends on them (types.md).
- A tag that does not match the schema, a missing required component and an unlisted component have no error class,
  so the adapter reports them as an adapter failure (a disagreement); no case reaches them.
- The Annex A.1 `children` DEFAULT `{}` is modelled as OPTIONAL: no case encodes an empty `children`.
- The adapter checks the SET order of a component before the contents of the next one, but validates the whole
  structure first (`Tlv::validate`); an input with both an order fault and a later structural fault would report the
  structural one. No case has both.
- `utctime-pivot`: a case that states a UTCTime value names its `pivot` (`Case::pivot`, passed to `parse_utc_time`); a case without one is parsed with 50 and states no century.
- Not independently verified: the Annex A.1 schema and the 8.14 type definitions against the standard (itulib down).
- Value forms exist only for the types listed in `format.md` "Decoded values". DATE, TIME-OF-DAY, DATE-TIME, DURATION, TIME, OID-IRI,
  RELATIVE-OID-IRI and the four unchecked 8-bit strings stay tree-only: a value form for them is a later format bump.
- The character strings: the crate only validates; the runner decodes the validated octets with `std` (UTF-8, UTF-16BE, UCS-4BE). A fault in
  the crate's validator shows as a value case that no longer decodes (e1, e2), not as a wrong string.
- The `unused` bits of a BIT STRING value read as zero (format.md); the runner masks them itself for the primitive form, so `BitString::data`
  returning the pad bits as received is not tested for any value case; `ConstructedBitString::copy_data` is (h16).
- Not verified: no clause was re-read from X.690 in this session (`itulib` returned "no such table: meta"); the values rest on the clause
  texts in `docs/design/` and on the arithmetic shown in each case provenance, and are cross-checked by the Python reference decoder, which is
  my own reading, not an independent standard. The decimal REAL forms (ISO 6093) and the calendar are not read, as before.
