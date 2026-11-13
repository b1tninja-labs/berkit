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
