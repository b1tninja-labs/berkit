# Known disagreements between the corpus and the code

`tests/corpus.rs` passes only when the set of disagreeing case ids equals the ids listed here. A case
that starts agreeing, or a new disagreement, fails the test: edit this file in the same commit.

Classifications: CODE-BUG (the code is wrong, the clause is clear), CORPUS-BUG (the case is wrong),
POLICY-GAP (code and corpus follow different policies), FORMAT-GAP (the corpus format cannot state it),
ADAPTER-LIMIT (the test adapter cannot express it).

| Case id | Class | Reason |
|---|---|---|
