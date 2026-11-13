# berkit roadmap

A row is `[ok]` only when the named test exists, passes, and failed when the code it covers was deliberately
broken; `[unread]` when the test passes but the anchor could not be read from the standard; `[open]` when not done.
Per-module rows live in `crates/berkit/roadmap/<module>.md` and are assembled here.

## Policy decisions

The standard has no receiver-side error rules and sets no limits. These choices are ours, each recorded where the
standard is silent. They are defaults, open to change.

| Status | Policy | Why |
|---|---|---|
| [open] | BER decoding enforces the standard's "shall" value rules, including minimal INTEGER, OID subidentifier and tag-number forms. There is no lenient mode in version 1. | X.690 §8.3.2, §8.19.2 and §8.1.2.4.2 are "shall" with no sender's option; a lenient reader is a deliberate deviation, so it would be a separate, named option |
| [open] | DER decoding rejects every non-canonical encoding of §10 and §11. | X.690 states only the sender's rule; "reject" is derived from §7.1 with §10 and §11 |
| [open] | Limits (nesting depth, length, tag number) are caller-supplied with documented defaults, and `limit_exceeded` reports them. | The standard sets no bound |
| [open] | Equal adjacent elements of a DER SET OF are accepted. | §11.6 says "ascending" and is silent on duplicates |
| [open] | SET and SET OF both use tag `0x31`; the ordering check is applied only when the caller names the type. | A TLV reader cannot tell them apart |
| [open] | UTCTime takes a century pivot as a parameter. | Neither X.680 nor X.690 gives one; RFC 5280 §4.1.2.5.1 gives 49/50 for PKIX |
| [open] | GeneralizedTime follows X.690 §11.7 (fractions allowed, trailing zero not), not the RFC 5280 profile, which is a later layer. | The standard's own valid example has a fraction |
| [open] | REAL contents are carried unchanged and re-encode byte for byte; no float conversion. | X.680 §21.3 makes base-2 and base-10 values distinct; an `f64` cannot hold -0, infinities or NaN |
| [open] | Leftover input after the single top-level encoding is `trailing_data`. | A design decision of this crate; the standard describes one encoding |
| [open] | Teletex, Videotex, Graphic and General strings are not alphabet-checked. | Their alphabets are ISO register entries that are not in the corpus |

## Modules

Each module's rows are in `crates/berkit/roadmap/<module>.md`. Tests are the integration tests of that module's target.

| Module | Source | Roadmap | `[ok]` | `[unread]` | `[open]` | Tests |
|---|---|---|---|---|---|---|
| framing (`tag`, `length`, `tlv`) | X.690 §8.1, §10.1 | `framing.md` | 1 | 26 | 10 | 38 |
| numeric (`boolean`, `integer`, `null`, `real`) | X.690 §8.2 to §8.8, §11.1, §11.3 | `numeric.md` | 30 | 2 | 4 | 41 |
| `oid` | X.690 §8.19 to §8.22 | `oid.md` | 11 | 2 | 3 | 27 |
| `bitstring`, `octetstring` | X.690 §8.6, §8.7, §10.2, §11.2 | `bitoctet.md` | 16 | 0 | 8 | 29 |
| `strings`, `time` | X.690 §8.23 to §8.26, §11.4, §11.7 to §11.9; X.680 §41 to §47 | `strtime.md` | 31 | 3 | 14 | 39 |
| corpus runner | `corpus/` 0.1.0 | `corpus.md` | 9 | 0 | 2 | 14 |

## Across the modules

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [unread] | The 26 framing rows rest on the research notes, not on a clause read at implementation time | `docs/design/framing.md` | the framing agent could not read X.690: the `itulib` index failed with "no such table: meta". Re-read each clause through the primarysource server and flip the rows |
| [ok] | The corpus runs against the code: 931 cases, 1859 runs, all agree, with a known-disagreement list that is now empty | `corpus/` 0.1.0 | `tests/corpus.rs::corpus_agrees_with_the_code_except_the_known_list`; breaking INTEGER minimality produced 7 disagreements, DER indefinite length 20, the PrintableString alphabet 183 |
| [open] | Agreement is code against corpus, not either against the standard: the corpus expectations are an author's reading of the clauses, written from the same research notes the code was built from, and checked by a Python reference decoder that is not independent of them | none | an independent check (a second implementation written without the notes, or a real-certificate sample) is not done |
| [open] | The corpus cannot state a decoded typed value, so decode cases do not pin it: breaking the OID first-arc split on decode (`v - 40` to `v - 41`) failed 5 module tests and no corpus case | `corpus/format.md` (format 1.1) | add an outcome that names the decoded value (the `typed.value` forms) and cases for each primitive, in a minor format bump |
| [open] | A typed layer in the library: the adapter that joins the TLV reader to the content functions lives in `tests/common/typed.rs` | none | promote it to a module once the corpus is settled |
| [open] | Reference material is missing: X.660 (OID arcs), ISO 6093 (REAL decimal), ISO/IEC 10646 (UTF-8, BMP and UCS-4 rules), ISO 8601 (time), the ISO register (Teletex, Videotex, Graphic, General) | requested from the primarysource coordinator | those rules stay `[unread]` |
| [open] | The crate builds on Rust 1.85 and on a bare-metal `no_std` target, and on 32-bit | `docs/msrv.md` | not tried: only stable 1.99 is installed |
| [open] | berkit is registered in the portfolio and has a remote | `PORTFOLIO.md` | not done |
