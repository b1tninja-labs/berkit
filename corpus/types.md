# Types defined by the corpus (`corpus:<Name>`)

Cases whose outcome needs a little schema knowledge name one of these types. They are not in the standard; each is
written here in ASN.1 so that a typed runner builds the same check. The clauses a case cites say what the rule is.

| Name | Definition | Used for |
|---|---|---|
| `corpus:ExplicitInt2` | `[2] EXPLICIT INTEGER` | X.690 §8.14.3: constructed, holds the whole INTEGER TLV; primitive is `wrong_form` |
| `corpus:ImplicitInt2` | `[2] IMPLICIT INTEGER` | X.690 §8.14.4: primitive, the INTEGER tag replaced; constructed is `wrong_form` |
| `corpus:DefaultSeq` | `SEQUENCE { a INTEGER DEFAULT 5 }` | X.690 §11.5: a present component equal to its DEFAULT is `default_encoded` in DER |
| `corpus:DefaultSeqTagged` | `SEQUENCE { a [0] IMPLICIT INTEGER DEFAULT 5, b [1] IMPLICIT INTEGER OPTIONAL }` | the same, with tags (an untagged form would be ambiguous, X.680 §25.6.1) |
| `corpus:NamedBitString` | `BIT STRING { a(0), b(1), c(2), d(3), e(4), f(5), g(6), h(7), i(8) }` (a named-bit list) | X.690 §11.2.2 via X.680 §22.7: DER removes trailing 0 bits; an all-zero value is `03 01 00` |

The cases that name only `SET` and `SET OF` (no component schema) use the universal types of that name: the caller
says which ordering rule to apply (ROADMAP policy `set-vs-setof`).
`SEQUENCE {...}` types written inline in a `type` field (for example `SEQUENCE {name IA5String, ok BOOLEAN}`)
only tell a runner that the top-level element is a SEQUENCE; no case depends on their component types.
