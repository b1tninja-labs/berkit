# Corpus runner roadmap (tests/corpus.rs, tests/common/, tests/corpus_known.md)

The runner reads `corpus/` (0.1.0, 931 cases, 5 files) and runs every case, in every mode it names, against the crate.
The `itulib` index was unavailable in this session (`itu_read` returned "no such table: meta"), so no clause was
re-read from X.690; the only classification so far is a design question, not a clause question. A breakage `M n`
was restored from a copy kept outside the repository and verified with `diff` and `git status`; `cargo clean -p berkit`
ran between runs.

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | All 931 cases run, once per mode the case names: 1859 runs (decode 643 ber + 643 der, roundtrip 244 ber + 243 der, encode 44 ber + 42 der); 815 cases are typed, 475 carry a `policy` | `corpus/format.md` Operations | `corpus_agrees_with_the_code_except_the_known_list` (prints the table with `--nocapture`); M1 `boolean.rs` DER check off (2 new), M2 Printable alphabet off (183 new disagreements), M3 `time.rs` DER zone check off (1 new), M4 `tlv.rs` DER indefinite check off (20 new) |
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
- `utctime-pivot`: the pivot 50 is passed, and the cases do not assert a century.
- Not independently verified: the Annex A.1 schema and the 8.14 type definitions against the standard (itulib down).
