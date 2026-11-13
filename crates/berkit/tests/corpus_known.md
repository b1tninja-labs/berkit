# Known disagreements between the corpus and the code

`tests/corpus.rs` passes only when the set of disagreeing case ids equals the ids listed here. A case
that starts agreeing, or a new disagreement, fails the test: edit this file in the same commit.

Classifications: CODE-BUG (the code is wrong, the clause is clear), CORPUS-BUG (the case is wrong),
POLICY-GAP (code and corpus follow different policies), FORMAT-GAP (the corpus format cannot state it),
ADAPTER-LIMIT (the test adapter cannot express it).

| Case id | Class | Reason |
|---|---|---|
| `x690.8.1.3.encode-length-nonminimal-81-01` | POLICY-GAP | The case wants `04 81 01 41` from a tree that names one long-form length octet for length 1 (X.690 8.1.3.5 NOTE 2 lets a BER sender choose it). `tlv::write_header` and `Length::encode` write the minimal form only and have no way to ask for surplus length octets; no ROADMAP row says the writer is minimal-only, so the gap is a decision for the coordinator (add a writer option, or retire the case as DER-only). The standard does not require a writer to offer it, so this is not a CODE-BUG. |
