# Design ideas worth keeping from asn1core (ideas only, no code)

For each: which clause makes it necessary, or "design choice".

1. **Every node keeps the exact octets it was parsed from.** Necessary: X.690 §7.3 ¶1 (BER offers
   alternative encodings as sender's options, so re-encoding need not reproduce the input) and RFC 5280
   §4.1.1.3 (signature over the DER of the TBS). Even DER decoding obeys §7.4 ¶1 only if the input
   was DER; the crate cannot assume it. Rust form: nodes borrow the input (`&'a [u8]` slices for the whole
   TLV, the content, and each child), so signature checking and SPKI hashing take slices, never a re-encoding.
2. **Typed hazard errors with a rule name and offset** (`Truncated`, `Malformed`, `DerViolation{rule, offset}`,
   `DepthExceeded`). Design choice, but it makes strict-vs-lenient testable and logs useful. The rule vocabulary
   is shared between the error and the defect-as-data (idea 3).
3. **Defects as data.** The lenient reader keeps the octets and returns `Defect{rule, message, offset}`
   instead of failing; a verifier decides. Design choice. The standard is silent on out-of-alphabet strings
   (X.680 §41.4 defines the type by its characters), so this is a policy layer, but it should stay
   separate from spelling rules (below).
4. **Two kinds of fault must not be merged.** (a) alternative *spelling* of a valid value: BER allows, DER forbids
   (§10.1, §10.2, §11.1, §11.2, §11.6); (b) *non-value*: no encoding of any value (non-minimal INTEGER, wrong bit 6,
   bad BIT STRING initial octet, §8.3.2, §8.2.1). Correction to asn1core: kind (b) is invalid in BER too, so a
   three-level model is clearer: Invalid / BER-only (sender options) / DER.
5. **Parse by objects, never by byte scanning; bound every child by its parent's limit** (§8.1.5, §8.1.3.6,
   §8.9.2). Necessary for indefinite lengths; the single worst snmpy defect.
6. **Boundary-case corpus, expectations derived from the rule, not from output**: tag numbers 30/31/127/128/16383/16384,
   lengths 127/128/255/256/65535/65536, `-128`, `00 80`, `FF 7F`. Plus an independent second implementation. Design
   choice, backed by §8.1.2.4.2, §8.1.3.5, §8.3.2/3.
7. **Mutation tests must keep enclosing lengths consistent**, or the framing error masks the rule under test.
8. **`Tag` as (class, constructed, number)** with number unbounded in principle and a stated cap (§8.1.2.4.2 allows
   any size; X.680 Table 1 has universal numbers above 30).
9. **Length, tag and OID encoders built on integer bit counts, not floats** (§8.1.3.5 ¶4); share one base-128 routine
   between tag numbers (§8.1.2.4.2) and subidentifiers (§8.19.2) and test it once.
10. **Check at the parent, not the child, for SET OF** (ordering is a property of the constructed value, §11.6) and
    keep SET (§10.3) separate. Fix: carry "SET vs SET OF" from the schema/typed layer rather than guessing from
    tag 17.
11. **Typed layer locates fields by tag, not index**; skip optional/DEFAULT members by tag (§11.5, §8.14). Design.
12. **Unknown tags and extensions survive decode-encode verbatim** (design; needed because extension contents are
    OCTET STRING wrapping, §8.7).
13. **Public API shape that made the invariant checkable**: `certificate_parts` returns slices of the original
    buffer (`tbs`, `spki`, signature octets) and a test verifies a real CA signature over exactly those slices;
    the parts include the stored TLV (tag and length included) and the signature value with the BIT STRING initial
    octet removed and required to be 0 (§8.6.2.2 ¶1).
14. **Depth, length and arc ceilings as explicit, named, caller-tunable limits** (all are policy; the standard sets none).
15. **A named test that failed first for each defect** and a "no assert on wire data" structural test. In Rust:
    forbid `unwrap`/`expect`/indexing/unchecked arithmetic on input paths (clippy lints), property tests and fuzz.
16. **DER-violation report as data** (`der_violations`): list rule and offset; improve by collecting all.
17. **Name each rule once** (`tag-number-not-minimal`, `length-not-minimal`, `integer-not-minimal`,
    `oid-arc-nonminimal`, `set-of-unsorted`, `constructed-string`, `indefinite-length`, ...) so strict, lenient and
    documentation share one vocabulary.
18. **Reject-by-name for what you do not model** (unrestricted CHARACTER STRING, unknown string tags) instead of
    guessing a codec.

Ideas not to inherit: schema-less `check_set_of` on all tag-17 nodes; value checks only on the DER path;
Python-style tolerant `decode_oid` used for BER; 8-octet length cap; `datetime`-bound time handling; first-violation-only
reporting.
