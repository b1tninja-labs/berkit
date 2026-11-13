# X.690 primitive value types: obligation table and test vectors

Sources: Rec. ITU-T X.690 (02/2021) | ISO/IEC 8825-1:2021 (cited "X.690"), Erratum 1 (09/2021), Rec. ITU-T X.680 (02/2021) (cited "X.680"), X.690 (11/2008) for the edition comparison.
Citation `§n ¶k` means the k-th sentence of the clause as extracted by `itu_read` (sentence numbering is the tool's, not the standard's; two sentences of 8.2.2 are a list stem plus item).
Tables were checked against `pdftotext -raw` output (scratch: `x690-2021-02.txt`, `x680.txt` in this directory). Erratum 1 only replaces Figure 4 (identifier octets, high tag number) and touches nothing here.

Conventions: "BER decode" = what a conforming BER receiver must do (X.690 §7.3: receivers shall support all sender alternatives). "DER decode" = what a DER receiver must reject (X.690 §7.4: no alternative encodings in CER/DER, so every sender-option the DER clauses remove is a reject). Where X.690 states only a sender constraint and the reject rule is my deduction, the row says "derived".

Tags (X.680 §8.6 Table 1, confirmed in pdftotext; per-type clauses 18.2, 19.8, 22.8, 23.2, 24.2, 32.2, 21.2, 20.7, 33.2, 34.2, 35.2): BOOLEAN 1 (01), INTEGER 2 (02), BIT STRING 3 (03; constructed 23), OCTET STRING 4 (04; constructed 24), NULL 5 (05), OBJECT IDENTIFIER 6 (06), REAL 9 (09), ENUMERATED 10 (0A), RELATIVE-OID 13 (0D), OID-IRI 35 and RELATIVE-OID-IRI 36 (identifier octets `1F 23` and `1F 24`, because tag numbers >= 31 take the multi-octet form, X.690 §8.1.2.4 ¶1; 0x23 and 0x24 are the base-128 tag value octets). Constructed form sets bit 6 (X.690 §8.1.2.5 ¶1).

## Error classes (names to use in the crate)

| Class | Meaning |
|---|---|
| `TruncatedInput` | fewer octets than the length requires (transport-level, not a standard rule) |
| `WrongForm` | constructed bit set where the type must be primitive, or constructed in DER where forbidden |
| `IndefinitePrimitive` | indefinite length on a primitive encoding (derived from §8.1.3.2 ¶1a, §8.1.3.6) |
| `BadLength` | content length violates the type's length rule (BOOLEAN != 1, NULL != 0, empty INTEGER...) |
| `NonMinimal` | a minimal-form rule violated (INTEGER, OID subidentifier, DER length) |
| `NonCanonical` | DER-only: a legal BER sender option that DER removes (BOOLEAN != FF, nonzero pad bits, REAL form) |
| `BitStringUnusedRange` | initial octet > 7 |
| `BitStringUnusedEmpty` | initial octet != 0 with no following octets |
| `BitStringSegment` | constructed segment violates §8.6.4 (non-final segment not a multiple of 8 bits, wrong inner tag) |
| `OidTruncated` | last octet of a subidentifier has bit 8 set |
| `OidEmpty` | OID / RELATIVE-OID with no content (derived) |
| `RealReserved` | reserved REAL first-octet value or malformed exponent field |
| `InvalidUtf8` | OID-IRI contents not well-formed UTF-8 |
| `Unsupported` | a valid encoding the implementation chooses not to represent (size limit); never silently wrong |

## 1. Obligations

`E` = encoder rule, `B` = BER decoder, `D` = DER decoder. "n/a" = not applicable.

### 1.0 Shared framing (owned by another slice, listed because every primitive depends on it)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| G1 | X.690 §8.1.2.5 ¶1 | Bit 6 of the identifier is 0 for primitive, 1 for constructed | primitive types always bit 6 = 0 | reject constructed for types whose clause says "shall be primitive" | same | `WrongForm` |
| G2 | X.690 §8.1.3.2 ¶1 a) | A sender uses the definite form for a primitive encoding | definite length | reject indefinite on primitive (derived from a) and §8.1.3.6 ¶1 requiring EOC, which a primitive has no way to carry) | same | `IndefinitePrimitive` |
| G3 | X.690 §8.1.3.5 ¶2 c) | Long-form initial octet 0xFF shall not be used | never emit | reject | reject | `BadLength` |
| G4 | X.690 §8.1.3.5 NOTE 2 | In the long form a sender may use more length octets than necessary | emit minimal | accept (e.g. `81 00`, `82 00 01`) | see G5 | n/a |
| G5 | X.690 §10.1 ¶1 | DER: definite form, minimum number of octets | short form <= 127, else fewest long-form octets | n/a | reject non-minimal long form and indefinite | `NonMinimal` / `WrongForm` |
| G6 | X.690 §7.3 | Receivers claiming BER shall support all sender alternatives | n/a | accept every alternative listed below | n/a | n/a |
| G7 | X.690 §7.4 | No alternative encodings in CER/DER | n/a | n/a | reject any BER-only alternative | `NonCanonical` |
| G8 | X.690 §8.1.2.4 ¶1 | Tag numbers >= 31 use a leading octet plus subsequent octets | OID-IRI and RELATIVE-OID-IRI identifiers are `1F 23`, `1F 24` | decode as high-tag-number | same | n/a |

### 1.1 BOOLEAN (X.690 §8.2, §11.1)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| BOOL1 | §8.2.1 ¶1 | Encoding is primitive | `01` | reject `21` | reject | `WrongForm` |
| BOOL2 | §8.2.1 ¶2 | Contents is exactly one octet | length 1 | reject length 0 or >= 2 | reject | `BadLength` |
| BOOL3 | §8.2.2 ¶2 | FALSE is the octet zero | `00` | `00` = FALSE | `00` = FALSE | n/a |
| BOOL4 | §8.2.2 ¶3 | TRUE is any non-zero value, sender's option | emit `FF` (single choice satisfying both BER and DER) | every non-zero octet = TRUE | see BOOL5 | n/a |
| BOOL5 | §11.1 ¶1 | DER/CER: TRUE has all eight bits set | `FF` | n/a | contents other than `00` or `FF` rejected (`00` from BOOL3, `FF` from BOOL5; derived: any other value is a BER-only alternative, §7.4) | `NonCanonical` |

### 1.2 INTEGER (X.690 §8.3) and ENUMERATED (§8.4)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| INT1 | §8.3.1 ¶1 | Primitive | `02` | reject `22` | reject | `WrongForm` |
| INT2 | §8.3.1 ¶2 | Contents is one or more octets | never empty | reject length 0 | reject | `BadLength` |
| INT3 | §8.3.3 ¶1, ¶4 | Contents is a two's-complement big-endian number; bit 8 of first octet is the sign (weight -2^(8n-1)) | sign-extend as needed | decode as signed | same | n/a |
| INT4 | §8.3.2 ¶1 a), b) | With more than one octet, the first octet plus bit 8 of the second shall not be all ones and shall not be all zero (NOTE: smallest possible number of octets) | emit shortest | the standard states this as a "shall not", not a sender's option: reject `00 00`, `00 7F`, `FF 80`, `FF FF`. A lenient mode that accepts them is a deliberate deviation and must be opt-in | reject | `NonMinimal` |
| INT5 | X.680 §3.8.48 ¶2-¶3 | INTEGER is every whole number; encoding rules may limit the range, chosen not to affect users | arbitrary precision on the encode side | no size limit is stated by X.690. Decide: store as big-endian two's-complement octets (lossless), with convenience accessors to i64/u64/i128 that fail with `Unsupported` | same | `Unsupported` (accessor only) |
| ENUM1 | §8.4 ¶1 | Encoding is that of the associated integer value | identical to INT rules, tag `0A` | identical, including INT4 | identical | as INT |
| ENUM2 | §8.4 ¶2 (NOTE) | It is primitive | `0A` | reject `2A` | reject | `WrongForm` |
| ENUM3 | X.680 §20.2-§20.3 | Enumeration numbers are integers (SignedNumber, may be negative); the number-to-name mapping is schema-level | n/a (schema) | TLV level returns the integer; unknown numbers are an application concern (X.680 extension handling, out of this slice) | same | n/a |

### 1.3 REAL (X.690 §8.5, §11.3)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| REAL1 | §8.5.1 ¶1 | Primitive | `09` | reject `29` | reject | `WrongForm` |
| REAL2 | §8.5.2 ¶1 | Plus zero has no contents octets | `09 00` | length 0 = +0 | same | n/a |
| REAL3 | §8.5.3 ¶1, §8.5.9 ¶1 | Minus zero is the single octet `43` | `09 01 43` | decode as -0 | same | n/a |
| REAL4 | §8.5.6 ¶1 | First-octet bit 8 = 1: binary; bits 8-7 = 00: decimal; bits 8-7 = 01: special value or minus zero | n/a | classify by first octet | same | n/a |
| REAL5 | §8.5.9 ¶1 | Bits 8-7 = 01: exactly one contents octet: `40` +INF, `41` -INF, `42` NaN, `43` -0. Other values with bits 8-7 = 01 are reserved | write only these four | reject other `01xxxxxx` values and any length != 1 | same | `RealReserved` / `BadLength` |
| REAL6 | §8.5.7 ¶1 | Binary: M = S x N x 2^F, 0 <= F < 4 | see DER row | decode fields per REAL7-REAL10 | same | n/a |
| REAL7 | §8.5.7.1 ¶1 | Bit 7 = 1 iff S = -1 | | sign | same | n/a |
| REAL8 | §8.5.7.2 ¶1-¶2 | Bits 6-5: 00 base 2, 01 base 8, 10 base 16, 11 reserved | | accept 00, 01, 10; reject 11 | same | `RealReserved` |
| REAL9 | §8.5.7.3 ¶1 | Bits 4-3 = F, unsigned | | read F (0..3) | same | n/a |
| REAL10 | §8.5.7.4 ¶1 a)-d) | Bits 2-1: 00: exponent in 1 octet; 01: 2 octets; 10: 3 octets (all two's complement); 11: next octet is the exponent length X (>= 1, unsigned), then X octets of exponent. In form 11 the first nine bits of the exponent shall not be all zeros or all ones | | follow; reject X = 0; reject exponent whose first nine bits are all equal (when X >= 2). Edge not stated: with X = 1 the exponent has only eight bits, so "first nine bits" cannot be checked; treat as unspecified, accept | same | `RealReserved` |
| REAL11 | §8.5.7.5 ¶1 | Remaining octets are N, unsigned big-endian | | N as octet string | same | n/a |
| REAL12 | §8.5.7.5 ¶2-¶3 (NOTE 1) | BER has no mantissa normalisation requirement | | accept any N, including leading zero octets and even N | see REAL13 | n/a |
| REAL13 | §11.3.1 ¶1-¶4 | DER, base-2 values: base 2 binary encoding; M odd (or 0); F = 0; M and E in the fewest octets | | n/a | reject bits 6-5 != 00, F != 0, N even, N with a leading `00` octet, exponent not in fewest octets (exponent two's-complement minimal; form 11 only when the exponent needs 4 or more octets: derived from "fewest octets") | `NonCanonical` |
| REAL14 | §8.5.8 ¶1-¶3 | Decimal: first octet bits 6-1 = `000001` NR1, `000010` NR2, `000011` NR3, others reserved; remaining octets are an ISO 6093 field of sender-chosen length | | accept NR1-NR3 (content not parsed in v1); reject other bits 6-1 | see REAL15 | `RealReserved` |
| REAL15 | §11.3.2 ¶1, §11.3.2.1-§11.3.2.6 | DER base-10 values: NR3 form, no SPACE, leading `-` only when negative, neither first nor last mantissa digit 0, mantissa then `.` then `E`, exponent `+0` when zero else no `+` and no leading zero digit | | n/a | reject bits 6-1 != `000011`; syntax checks above (ISO 6093 itself is not in the corpus; see section 4) | `NonCanonical` |
| REAL16 | §8.5.4 ¶1, §8.5.5 ¶1-¶3 | Abstract base 10 values use base 10 encoding; base 2 values use base 2, 8 or 16 (sender's option) | | a value cannot be flipped between base 10 and base 2 encodings without changing the abstract value (X.680 §21.3 ¶3) | same | n/a |

REAL scope decision for v1 is in section 5.

### 1.4 BIT STRING (X.690 §8.6, §11.2)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| BIT1 | §8.6.1 ¶1 | Primitive or constructed, sender's option | primitive by default | accept both | constructed rejected (BIT7) | n/a |
| BIT2 | §8.6.2 ¶1 | Primitive contents: an initial octet, then zero or more octets | always >= 1 content octet | reject length 0 | reject | `BadLength` |
| BIT3 | §8.6.2.1 ¶1 | Bits are placed leading bit first into bit 8 of the first data octet, downwards; the final data octet is filled from bit 8 | MSB-first packing | MSB-first unpacking | same | n/a |
| BIT4 | §8.6.2.2 ¶1-¶2 | Initial octet is the number of unused bits (0 to 7) in the final data octet | write bit_len mod 8 complement | reject > 7 | reject | `BitStringUnusedRange` |
| BIT5 | §8.6.2.3 ¶1 | Empty bit string: no data octets, initial octet zero | `03 01 00` | accept; with length 1, reject initial octet != 0 (derived from BIT4 + BIT5: no final data octet exists) | same | `BitStringUnusedEmpty` |
| BIT6 | §11.2.1 ¶1 | DER/CER: every unused bit in the final octet is 0 | zero the pad | BER does not constrain pad-bit values (nothing found); accept any, mask them off so equal values compare equal | reject non-zero pad bits | `NonCanonical` |
| BIT7 | §10.2 ¶1 | DER: constructed form shall not be used for bit string | primitive | n/a | reject `23` | `WrongForm` |
| BIT8 | §8.6.2.4 ¶1-¶2, §11.2.2 ¶1-¶3 (via X.680 §22.7) | Only when the type has a NamedBitList: BER may add or remove trailing 0 bits; DER shall remove all trailing 0 bits, and an all-zero value is `03 01 00` | schema-aware encoders strip trailing zeros in DER | accept any length (no stripping) | requires schema knowledge: a TLV-level decoder cannot decide. Provide a flag `named_bit_list`; when set, reject a value whose last bit is 0, and reject an all-zero value not encoded `03 01 00` | `NonCanonical` |
| BIT9 | §8.6.3 ¶1 | Constructed contents: zero, one or more nested encodings | | accept 0 segments (empty value) | reject (BIT7) | n/a |
| BIT10 | §8.6.4 ¶1-¶3 | Segmentation: consecutive bits; every segment except the last has a multiple of 8 bits; boundaries carry no meaning; each bit in exactly one segment | | concatenate segment bit-strings in order | n/a | `BitStringSegment` |
| BIT11 | §8.6.4 ¶4 (NOTE) | A segment may be empty (zero bits) | | accept `03 01 00` as a segment | n/a | n/a |
| BIT12 | §8.6.4.1 ¶1-¶3 | Each segment is encoded recursively as if it were a bit string, in value order; so each segment may itself be constructed | | accept nested constructed segments; the "multiple of eight" rule applies to a segment that is not last at its own level; a nested constructed non-final segment must in total hold a multiple of 8 bits | n/a | `BitStringSegment` |
| BIT13 | §8.6.4.1 ¶6 (NOTE 2) | Inner tags are always universal 3 | | reject any other inner identifier (NOTE, but the recursion treats segments as bit strings) | n/a | `BitStringSegment` |
| BIT14 | §8.1.3.2 ¶1 b)-c), §8.1.5 ¶1-¶2 | A constructed encoding uses definite or indefinite length at the sender's option; indefinite ends with `00 00` | | accept both | n/a | n/a |

### 1.5 OCTET STRING (X.690 §8.7)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| OCT1 | §8.7.1 ¶1 | Primitive or constructed, sender's option | primitive | accept both | constructed rejected (OCT6) | n/a |
| OCT2 | §8.7.2 ¶1 | Primitive: zero or more contents octets equal to the value's octets | `04 00` for empty | length 0 = empty | same | n/a |
| OCT3 | §8.7.3 ¶1 | Constructed: zero or more nested encodings | | accept 0 segments (empty) | reject | n/a |
| OCT4 | §8.7.3.1 ¶1-¶4 | Segments are consecutive octets; boundaries carry no meaning; a segment may be empty | | concatenate | n/a | n/a |
| OCT5 | §8.7.3.2 ¶1-¶3, ¶6 | Each segment recursively encoded as an octet string, in value order; inner tags are universal 4 (NOTE 2) | | accept nested constructed; reject other inner identifiers | n/a | `WrongForm` |
| OCT6 | §10.2 ¶1 | DER: constructed form shall not be used | primitive | n/a | reject `24` | `WrongForm` |

### 1.6 NULL (X.690 §8.8)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| NULL1 | §8.8.1 ¶1 | Primitive | `05` | reject `25` | reject | `WrongForm` |
| NULL2 | §8.8.2 ¶1-¶2 | No contents octets, length zero | `05 00` | reject length >= 1 | reject | `BadLength` |

### 1.7 OBJECT IDENTIFIER (X.690 §8.19)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| OID1 | §8.19.1 ¶1 | Primitive | `06` | reject `26` | reject | `WrongForm` |
| OID2 | §8.19.2 ¶1-¶2 | Contents is the concatenation of subidentifier encodings; each is one or more octets | | split on octets with bit 8 = 0 | same | n/a |
| OID3 | §8.19.2 ¶3 | Bit 8 is 1 on every octet except the last of a subidentifier, where it is 0 | | a final octet with bit 8 = 1 means the subidentifier never ends: reject | same | `OidTruncated` |
| OID4 | §8.19.2 ¶4-¶5 | Bits 7-1 of the octets, concatenated most significant first, form an unsigned number | base-128, big-endian | unsigned, unbounded (no maximum stated; see OID8) | same | n/a |
| OID5 | §8.19.2 ¶6 | Fewest possible octets: the leading octet of a subidentifier shall not be `80` | never emit `80` first | stated as "shall", not a sender's option: reject a subidentifier whose first octet is `80` (in BER too) | reject | `NonMinimal` |
| OID6 | §8.19.3 ¶1 | Number of subidentifiers N = (number of components) - 1 | | | | n/a |
| OID7 | §8.19.4 ¶1-¶2 | First subidentifier = X*40 + Y, X first component, Y second | encode two arcs as one subidentifier | decode: read first subidentifier v; X, Y recovered as below | same | n/a |
| OID7a | derived from §8.19.4 ¶2-¶3 NOTE (the 0/1/2 and "at most 39" facts are only in that NOTE and in X.660, not in the corpus) | Split: v < 40 gives (0, v); v < 80 gives (1, v - 40); otherwise (2, v - 80). The second arc is unbounded only when X = 2 | encode: first arc must be 0, 1 or 2; if first arc < 2 the second must be <= 39 | apply the split; v can be arbitrarily large (e.g. 2.999) | same | `Unsupported` only for size limits |
| OID8 | not found in X.690 | No upper bound on a subidentifier value or on the count of arcs is stated | arbitrary-precision API | decide: keep arcs as big-endian minimal octets or a bigint; do not truncate silently. Hard resource limit allowed only if reported as `Unsupported` | same | `Unsupported` |
| OID9 | derived from §8.19.3 ¶1 and X.680 §32.11 ¶2 (NOTE: at least two arcs) | Empty contents has no first subidentifier, so is not a value | | reject length 0 | reject | `OidEmpty` |
| OID10 | §8.19.5 ¶2-¶4 (example) | {2 999 3}: first subidentifier 1079 = `88 37`, then `03` | | | | n/a |

### 1.8 RELATIVE-OID (X.690 §8.20)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| ROID1 | §8.20.1 ¶1 | Primitive | `0D` | reject `2D` | reject | `WrongForm` |
| ROID2 | §8.20.2 ¶1-¶5 | Concatenated sub-identifiers, same base-128 form as OID2-OID4 | | as OID2-OID4 | same | `OidTruncated` |
| ROID3 | §8.20.2 ¶6 | Leading octet of a sub-identifier shall not be `80` | | reject | reject | `NonMinimal` |
| ROID4 | §8.20.3 ¶1, §8.20.4 ¶1 | N sub-identifiers = N arcs; the i-th is the i-th arc; no first-two-arc packing | one subidentifier per arc | each is its own arc, including the first (a first value >= 40 is legal) | same | n/a |
| ROID5 | derived from X.680 §33.3 ¶3 (the component list has at least one component) | Empty contents is not a value of the notation | | reject length 0 | reject | `OidEmpty` |
| ROID6 | X.680 §33.6 ¶4 | The starting node is neither the root nor directly beneath it: this is a semantic fact about use, not a decode rule | | none | none | n/a |

### 1.9 OID-IRI and RELATIVE-OID-IRI (X.690 §8.21, §8.22)

| ID | Clause and sentence | Rule | Encode | Decode in BER | Decode in DER | Error class |
|---|---|---|---|---|---|---|
| IRI1 | §8.21.1 ¶1 | Primitive | `1F 23` | reject `3F 23` | reject | `WrongForm` |
| IRI2 | §8.21.2 ¶1 | Contents are the UTF-8 of the lexical items of the XML value notation (X.680 §34.3), no white-space between items. That XML notation is `FirstArcIdentifier SubsequentArcIdentifier`, i.e. it starts with `/` and has no quotation marks | write UTF-8 of `/arc/arc...` | no X.690 rule on grammar; v1 checks UTF-8 only (see IRI3) | same | `InvalidUtf8` |
| IRI3 | §8.21.2 ¶2 | No announcers or escape sequences; each character in the smallest number of octets | shortest-form UTF-8 | reject overlong or otherwise ill-formed UTF-8 (the clause says smallest-octet encoding; surrogates, out-of-range code points are not addressed: use the ISO/IEC 10646 definition, not in corpus) | same | `InvalidUtf8` |
| IRI4 | X.680 §34.3 ¶3 | Arc labels are integer or non-integer Unicode labels (defined by X.660, not in the corpus) | | label validation deferred (v1: not done) | same | n/a |
| RIRI1 | §8.22.1 ¶1 | Primitive | `1F 24` | reject `3F 24` | reject | `WrongForm` |
| RIRI2 | §8.22.2 ¶1 | UTF-8 of the lexical items of X.680 §35.3: `FirstRelativeArcIdentifier SubsequentArcIdentifier`, first arc without a leading `/` | | as IRI2-IRI3 | same | `InvalidUtf8` |
| RIRI3 | derived | Clause §8.22.2 does not repeat the "smallest number of octets" sentence of §8.21.2 ¶2; it is still UTF-8, so apply IRI3 | | as IRI3 | same | `InvalidUtf8` |

Obligation count: 8 (G) + 5 (BOOL) + 8 (INT/ENUM) + 16 (REAL) + 14 (BIT) + 6 (OCT) + 2 (NULL) + 11 (OID, including OID7a) + 6 (ROID) + 7 (IRI/RIRI) = 83.

## 2. Test vectors

`hex input` is the full TLV. "BER" and "DER" modes decode with the rules of section 1. Where one input has different expectations by mode it is one row with both. Value notation: `bits(...)` for BIT STRING (leading bit first), `arcs(...)` for OIDs.

Verification: every encode-style vector marked "constructed" was regenerated by `a scratch script` from the clause rules (not from any implementation) and the output matches the hex below. Reject vectors are by-hand perturbations and were not executed against any decoder.

### 2.1 BOOLEAN

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V-B01 | `01 01 FF` | BER, DER | TRUE | from X.690 §8.2.2 example (contents `FF`) |
| V-B02 | `01 01 00` | BER, DER | FALSE | constructed by hand from §8.2.2 ¶2 (FALSE octet is zero; tag 01, length 01) |
| V-B03 | `01 01 01` | BER | TRUE | constructed by hand from §8.2.2 ¶3 (any non-zero octet is TRUE) |
| V-B04 | `01 01 01` | DER | `NonCanonical` | constructed by hand from §11.1 ¶1 (TRUE needs all eight bits set) |
| V-B05 | `01 01 80` | BER / DER | TRUE / `NonCanonical` | constructed by hand from §8.2.2 ¶3 and §11.1 ¶1 (0x80 non-zero but not all ones) |
| V-B06 | `01 00` | BER, DER | `BadLength` | constructed by hand from §8.2.1 ¶2 (zero contents octets) |
| V-B07 | `01 02 FF FF` | BER, DER | `BadLength` | constructed by hand from §8.2.1 ¶2 (two contents octets) |
| V-B08 | `21 03 01 01 FF` | BER, DER | `WrongForm` | constructed by hand from §8.2.1 ¶1 and §8.1.2.5 ¶1 (bit 6 set: 0x01 or 0x20 = 0x21) |
| V-B09 | `01 80 FF 00 00` | BER, DER | `IndefinitePrimitive` | constructed by hand from §8.1.3.2 ¶1 a) (indefinite length 80 on a primitive) |

### 2.2 INTEGER and ENUMERATED

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V-I01 | `02 01 00` | BER, DER | 0 | constructed by hand from §8.3.3 ¶1 (one octet 0x00 = 0) |
| V-I02 | `02 01 7F` | BER, DER | 127 | constructed by hand from §8.3.3 ¶4 (0x7F, bit 8 clear: 127) |
| V-I03 | `02 02 00 80` | BER, DER | 128 | constructed by hand from §8.3.3 ¶4 and §8.3.2 ¶1 (one octet 0x80 would be -128, so a leading 0x00 is needed; first octet zero but bit 8 of second is 1, so rule b) is not triggered) |
| V-I04 | `02 01 80` | BER, DER | -128 | constructed by hand from §8.3.3 ¶4 (0x80 = 128 - 256) |
| V-I05 | `02 02 FF 7F` | BER, DER | -129 | constructed by hand from §8.3.3 ¶4 (0xFF7F = 65407 - 65536 = -129). Minimal: bits tested are 1111 1111 and 0 (bit 8 of 0x7F), not all ones. Note: this is NOT the non-minimal case |
| V-I06 | `02 02 00 FF` | BER, DER | 255 | constructed by hand from §8.3.3 ¶4 (0x00FF) |
| V-I07 | `02 02 01 00` | BER, DER | 256 | constructed by hand from §8.3.3 ¶4 (0x0100) |
| V-I08 | `02 01 FF` | BER, DER | -1 | constructed by hand from §8.3.3 ¶4 (0xFF = 255 - 256) |
| V-I09 | `02 02 7F FF` | BER, DER | 32767 | constructed by hand from §8.3.3 ¶4 |
| V-I10 | `02 02 80 00` | BER, DER | -32768 | constructed by hand from §8.3.3 ¶4 (0x8000 = 32768 - 65536) |
| V-I11 | `02 03 00 80 00` | BER, DER | 32768 | constructed by hand from §8.3.3 ¶4 (0x8000 needs a leading 0x00) |
| V-I12 | `02 01 33` | BER, DER | 51 | from X.690 Annex A.3 (employee number content `33`, there under tag `42`) |
| V-I13 | `02 09 01 00 00 00 00 00 00 00 00` | BER, DER | 2^64 (18446744073709551616) | constructed by hand from §8.3.3 ¶4 (bit 64 set, nine octets, no sign padding needed) |
| V-I14 | `02 09 00 FF FF FF FF FF FF FF FF` | BER, DER | 2^64 - 1 | constructed by hand from §8.3.3 ¶4 and §8.3.2 ¶1 (eight 0xFF octets would read as negative, leading 0x00 is required) |
| V-I15 | `02 00` | BER, DER | `BadLength` | constructed by hand from §8.3.1 ¶2 (contents shall have one or more octets) |
| V-I16 | `02 02 00 7F` | BER, DER | `NonMinimal` (value would be 127) | constructed by hand from §8.3.2 ¶1 b) (first octet and bit 8 of second all zero) |
| V-I17 | `02 02 FF 80` | BER, DER | `NonMinimal` (value would be -128) | constructed by hand from §8.3.2 ¶1 a) (first octet 1111 1111 and bit 8 of second = 1: all ones) |
| V-I18 | `02 02 00 00` | BER, DER | `NonMinimal` | constructed by hand from §8.3.2 ¶1 b) |
| V-I19 | `02 02 FF FF` | BER, DER | `NonMinimal` | constructed by hand from §8.3.2 ¶1 a) |
| V-I20 | `02 03 00 00 80` | BER, DER | `NonMinimal` | constructed by hand from §8.3.2 ¶1 b) (extra leading zero over V-I03) |
| V-I21 | `02 02 01` | BER, DER | `TruncatedInput` | constructed by hand from §8.1.3.3 (length 2 but one octet present) |
| V-I22 | `22 03 02 01 00` | BER, DER | `WrongForm` | constructed by hand from §8.3.1 ¶1 |
| V-I23 | `02 81 01 05` | BER / DER | 5 / `NonMinimal` (length) | constructed by hand from §8.1.3.5 NOTE 2 and §10.1 ¶1 (long form where short suffices) |
| V-E01 | `0A 01 00` | BER, DER | ENUMERATED 0 | constructed by hand from §8.4 ¶1 (tag 0A, content as INTEGER 0) |
| V-E02 | `0A 01 02` | BER, DER | ENUMERATED 2 | constructed by hand from §8.4 ¶1 |
| V-E03 | `0A 01 FF` | BER, DER | ENUMERATED -1 | constructed by hand from §8.4 ¶1 and X.680 §20.3 (integer valued, may be negative) |
| V-E04 | `0A 00` | BER, DER | `BadLength` | constructed by hand from §8.4 ¶1 with §8.3.1 ¶2 |
| V-E05 | `0A 02 00 05` | BER, DER | `NonMinimal` | constructed by hand from §8.4 ¶1 with §8.3.2 ¶1 b) |

### 2.3 NULL

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V-N01 | `05 00` | BER, DER | NULL | from X.690 §8.8.2 example |
| V-N02 | `05 01 00` | BER, DER | `BadLength` | constructed by hand from §8.8.2 ¶1 (one content octet present) |
| V-N03 | `25 00` | BER, DER | `WrongForm` | constructed by hand from §8.8.1 ¶1 (0x05 | 0x20) |
| V-N04 | `05 81 00` | BER / DER | NULL / `NonMinimal` | constructed by hand from §8.1.3.5 NOTE 2 and §10.1 ¶1 |
| V-N05 | `05 80 00 00` | BER, DER | `IndefinitePrimitive` | constructed by hand from §8.1.3.2 ¶1 a) |

### 2.4 BIT STRING

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V-S01 | `03 01 00` | BER, DER | bits() (empty) | constructed by hand from §8.6.2.3 ¶1 |
| V-S02 | `03 02 00 FF` | BER, DER | bits(11111111) | constructed by hand from §8.6.2.1 ¶1 and §8.6.2.2 ¶1 (0 unused bits) |
| V-S03 | `03 02 07 80` | BER, DER | bits(1) | constructed by hand from §8.6.2.1 ¶1 (leading bit in bit 8) and §8.6.2.2 (7 unused bits) |
| V-S04 | `03 02 07 00` | BER, DER | bits(0) | constructed by hand from the same clauses (non-named type, so no §11.2.2 trailing-zero removal) |
| V-S05 | `03 02 04 F0` | BER, DER | bits(1111) | constructed by hand from §8.6.2.2 ¶1 (4 unused bits, all zero) |
| V-S06 | `03 02 04 F1` | BER / DER | bits(1111) with pad bit ignored / `NonCanonical` | constructed by hand from §11.2.1 ¶1 (unused bit 1 is set); BER silent on pad values, see section 4 |
| V-S07 | `03 02 08 00` | BER, DER | `BitStringUnusedRange` | constructed by hand from §8.6.2.2 ¶2 (range 0..7) |
| V-S08 | `03 02 FF 00` | BER, DER | `BitStringUnusedRange` | same |
| V-S09 | `03 01 03` | BER, DER | `BitStringUnusedEmpty` | constructed by hand from §8.6.2.3 ¶1 (empty value needs initial octet zero; here claims 3 unused bits of a nonexistent octet) |
| V-S10 | `03 00` | BER, DER | `BadLength` | constructed by hand from §8.6.2 ¶1 (initial octet missing) |
| V-S11 | `03 07 04 0A 3B 5F 29 1C D0` | BER, DER | bits(0000 1010 0011 1011 0101 1111 0010 1001 0001 1100 1101), 44 bits | from X.690 §8.6.4.2 example (value '0A3B5F291CD'H, primitive) |
| V-S12 | `23 80 03 03 00 0A 3B 03 05 04 5F 29 1C D0 00 00` | BER / DER | same 44-bit value / `WrongForm` | from X.690 §8.6.4.2 example (constructed, indefinite); DER by §10.2 ¶1 |
| V-S13 | `23 0C 03 03 00 0A 3B 03 05 04 5F 29 1C D0` | BER / DER | same 44-bit value / `WrongForm` | constructed by hand from §8.1.3.2 ¶1 b) and V-S12 (definite length: segments 5 + 7 = 12 = 0x0C octets) |
| V-S14 | `23 00` | BER / DER | bits() / `WrongForm` | constructed by hand from §8.6.3 ¶1 (zero nested encodings) |
| V-S15 | `23 80 00 00` | BER / DER | bits() / `WrongForm` | same with indefinite length (§8.1.5 ¶2 EOC `00 00`) |
| V-S16 | `23 80 03 01 00 03 02 00 FF 00 00` | BER | bits(11111111) | constructed by hand from §8.6.4 ¶4 NOTE (an empty segment is allowed) |
| V-S17 | `23 80 03 02 04 F0 03 02 00 FF 00 00` | BER | `BitStringSegment` | constructed by hand from §8.6.4 ¶2 (first segment is not last and has 4 unused bits, so is not a multiple of 8 bits) |
| V-S18 | `23 80 23 80 03 02 00 FF 00 00 03 02 00 AA 00 00` | BER | bits(11111111 10101010) | constructed by hand from §8.6.4.1 ¶1-¶4 (outer constructed; first segment is itself constructed `23 80 03 02 00 FF 00 00`; second segment `03 02 00 AA`; outer EOC is the final `00 00`) |
| V-S19 | `23 06 04 04 00 0A 3B 5F` | BER | `BitStringSegment` | constructed by hand from §8.6.4.1 ¶6 (inner tag 04 instead of 03) |
| V-S20 | `03 02 07 80` | DER with named bits | accept (bit 0 set, last bit is 1) | constructed by hand from §11.2.2 ¶1 |
| V-S21 | `03 03 07 80 00` | DER with named bits | `NonCanonical` (9 bits `100000000`, trailing zeros not removed) | constructed by hand from §11.2.2 ¶1 |
| V-S22 | `03 03 07 80 00` | BER | bits(100000000) accepted as is | constructed by hand from §8.6.2.4 ¶1 (add/remove of trailing zeros is permitted, not required) |
| V-S23 | `03 02 00 00` | DER with named bits | `NonCanonical` (8 zero bits, must be `03 01 00`) | constructed by hand from §11.2.2 ¶3 NOTE 2 |
| V-S24 | `03 01 00` | DER with named bits | accept, bits() | constructed by hand from §11.2.2 ¶3 NOTE 2 |

### 2.5 OCTET STRING

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V-O01 | `04 00` | BER, DER | empty | constructed by hand from §8.7.2 ¶1 (zero or more octets) |
| V-O02 | `04 03 01 02 03` | BER, DER | 01 02 03 | constructed by hand from §8.7.2 ¶1 |
| V-O03 | `24 80 04 02 01 02 04 01 03 00 00` | BER / DER | 01 02 03 / `WrongForm` | constructed by hand from §8.7.3.1-§8.7.3.2 (segments `01 02`, `03`) and §10.2 ¶1 |
| V-O04 | `24 07 04 02 01 02 04 01 03` | BER / DER | 01 02 03 / `WrongForm` | constructed by hand: definite length 4 + 3 = 7 |
| V-O05 | `24 80 24 80 04 01 AA 00 00 04 01 BB 00 00` | BER | AA BB | constructed by hand from §8.7.3.2 ¶1-¶4 (recursion) |
| V-O06 | `24 03 03 01 00` | BER | `WrongForm` | constructed by hand from §8.7.3.2 ¶6 (inner tag 03 not 04) |
| V-O07 | `24 00` | BER / DER | empty / `WrongForm` | constructed by hand from §8.7.3 ¶1 (zero encodings) |
| V-O08 | `24 80 04 00 04 01 07 00 00` | BER | 07 | constructed by hand from §8.7.3.1 ¶4 NOTE (empty segment) |

### 2.6 OBJECT IDENTIFIER and RELATIVE-OID

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V-D01 | `06 03 88 37 03` | BER, DER | arcs(2 999 3) | from X.690 §8.19.5 example (02/2021) |
| V-D02 | `06 03 81 34 03` | BER, DER | arcs(2 100 3) | from X.690 (11/2008) §8.19.5 example. Construction check: 2*40 + 100 = 180 = 1*128 + 52 = `81 34` (the first-arc-2 case with second arc above 39; this example is not in the 02/2021 text) |
| V-D03 | `06 02 2A 03` | BER, DER | arcs(1 2 3) | constructed by hand from §8.19.4 ¶2 (1*40 + 2 = 42 = 0x2A) |
| V-D04 | `06 01 00` | BER, DER | arcs(0 0) | constructed by hand from §8.19.4 ¶2 (0*40 + 0) |
| V-D05 | `06 01 27` | BER, DER | arcs(0 39) | constructed by hand from §8.19.4 ¶2 (39 = 0x27) |
| V-D06 | `06 01 28` | BER, DER | arcs(1 0) | constructed by hand from §8.19.4 ¶2 (40 = 0x28) |
| V-D07 | `06 01 4F` | BER, DER | arcs(1 39) | constructed by hand from §8.19.4 ¶2 (79 = 0x4F) |
| V-D08 | `06 01 50` | BER, DER | arcs(2 0) | constructed by hand from §8.19.4 ¶2 (80 = 0x50) |
| V-D09 | `06 01 7F` | BER, DER | arcs(2 47) | constructed by hand from §8.19.4 ¶2 (80 + 47 = 127 = 0x7F, largest single-octet first subidentifier) |
| V-D10 | `06 02 81 00` | BER, DER | arcs(2 48) | constructed by hand from §8.19.4 ¶2 and §8.19.2 (128 = 1*128 + 0 = `81 00`; first subidentifier 128 = 80 + 48) |
| V-D11 | `06 08 2A 86 48 86 F7 0D 01 01` | BER, DER | arcs(1 2 840 113549 1 1) | constructed by hand from §8.19.2 ¶2-¶5 (840 = 6*128 + 72 = `86 48`; 113549 = 6*16384 + 111*128 + 13 = `86 F7 0D`) |
| V-D12 | `06 03 2A 81 00` | BER, DER | arcs(1 2 128) | constructed by hand from §8.19.2 ¶4-¶5 (128 = `81 00`) |
| V-D13 | `06 06 2A 90 80 80 80 00` | BER, DER | arcs(1 2 4294967296) | constructed by hand from §8.19.2 (2^32: 33 bits, five 7-bit groups 16,0,0,0,0 = `90 80 80 80 00`) |
| V-D14 | `06 0B 2A 82 80 80 80 80 80 80 80 80 00` | BER, DER | arcs(1 2 18446744073709551616) | constructed by hand from §8.19.2 (2^64: 65 bits, ten groups 2,0,...,0). Must not truncate to 64 bits: decode as bigint or report `Unsupported` |
| V-D15 | `06 05 90 80 80 80 50` | BER, DER | arcs(2 4294967296) | constructed by hand from §8.19.4 (first subidentifier 80 + 2^32 = 4294967376 = `90 80 80 80 50`) |
| V-D16 | `06 03 2A 80 01` | BER, DER | `NonMinimal` | constructed by hand from §8.19.2 ¶6 (arc `80 01` has leading octet 80; the legal form of 1 is `01`) |
| V-D17 | `06 02 80 01` | BER, DER | `NonMinimal` | same, in the first subidentifier |
| V-D18 | `06 03 2A 86 48 86` | BER, DER | `OidTruncated` | constructed by hand from §8.19.2 ¶3 (last octet `86` has bit 8 set) |
| V-D19 | `06 02 2A 81` | BER, DER | `OidTruncated` | same |
| V-D20 | `06 00` | BER, DER | `OidEmpty` | constructed by hand, derived from §8.19.3 ¶1 and X.680 §32.11 ¶2 |
| V-D21 | `26 03 88 37 03` | BER, DER | `WrongForm` | constructed by hand from §8.19.1 ¶1 |
| V-D22 | `06 04 88 37 03 FF` | BER, DER | `OidTruncated` | constructed by hand from §8.19.2 ¶3 (final octet FF has bit 8 set) |
| V-R01 | `0D 04 C2 7B 03 02` | BER, DER | rel-arcs(8571 3 2) | from X.690 §8.20.5 example |
| V-R02 | `0D 01 00` | BER, DER | rel-arcs(0) | constructed by hand from §8.20.4 ¶1 |
| V-R03 | `0D 01 2A` | BER, DER | rel-arcs(42) (no first-two-arc packing) | constructed by hand from §8.20.3-§8.20.4 (contrast §8.19.4) |
| V-R04 | `0D 02 80 01` | BER, DER | `NonMinimal` | constructed by hand from §8.20.2 ¶6 |
| V-R05 | `0D 01 80` | BER, DER | `OidTruncated` | constructed by hand from §8.20.2 ¶3 |
| V-R06 | `0D 00` | BER, DER | `OidEmpty` | constructed by hand, derived from X.680 §33.3 ¶3 |

### 2.7 REAL (opaque handling; structural checks only)

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V-F01 | `09 00` | BER, DER | +0 | constructed by hand from §8.5.2 ¶1 |
| V-F02 | `09 01 40` | BER, DER | PLUS-INFINITY | constructed by hand from §8.5.9 ¶1 (`01000000`) |
| V-F03 | `09 01 41` | BER, DER | MINUS-INFINITY | same (`01000001`) |
| V-F04 | `09 01 42` | BER, DER | NOT-A-NUMBER | same (`01000010`) |
| V-F05 | `09 01 43` | BER, DER | minus zero | constructed by hand from §8.5.3 ¶1 and §8.5.9 ¶1 (`01000011`) |
| V-F06 | `09 01 44` | BER, DER | `RealReserved` | constructed by hand from §8.5.9 ¶1 (other `01xxxxxx` reserved) |
| V-F07 | `09 02 40 00` | BER, DER | `BadLength` | constructed by hand from §8.5.9 ¶1 (only one contents octet) |
| V-F08 | `09 03 80 00 01` | BER, DER | binary: S=+, base 2, F=0, E=0, N=1, value 1 | constructed by hand from §8.5.7-§8.5.7.5 (first octet: bit 8 = 1, bit 7 = 0, bits 6-5 = 00, bits 4-3 = 00, bits 2-1 = 00, so `80`; exponent `00`; N `01`) |
| V-F09 | `09 03 C0 FF 05` | BER, DER | binary: S=-, base 2, E=-1, N=5, value -2.5 | constructed by hand (`C0`: bit 7 = 1 negative; exponent `FF` = -1; 5 x 2^-1 = 2.5) |
| V-F10 | `09 03 80 00 02` | BER / DER | valid / `NonCanonical` (N even) | constructed by hand from §8.5.7.5 NOTE 1 and §11.3.1 ¶2 |
| V-F11 | `09 03 90 00 01` | BER / DER | valid (base 8) / `NonCanonical` | constructed by hand from §8.5.7.2 (bits 6-5 = 01) and §11.3.1 ¶1 |
| V-F12 | `09 03 84 00 01` | BER / DER | valid (F=1, value 2) / `NonCanonical` | constructed by hand from §8.5.7.3 and §11.3.1 ¶4 (F must be zero) |
| V-F13 | `09 04 81 00 01 01` | BER / DER | valid (exponent 2 octets `00 01`) / `NonCanonical` | constructed by hand from §8.5.7.4 b) and §11.3.1 ¶4 (exponent not in fewest octets) |
| V-F14 | `09 04 80 00 00 01` | BER / DER | valid (N = `00 01`) / `NonCanonical` | constructed by hand from §8.5.7.5 and §11.3.1 ¶4 (N not in fewest octets) |
| V-F15 | `09 03 B0 00 01` | BER, DER | `RealReserved` | constructed by hand from §8.5.7.2 (bits 6-5 = 11 reserved) |
| V-F16 | `09 05 83 02 00 01 01` | BER, DER | `RealReserved` | constructed by hand from §8.5.7.4 d) (form 11, X=2, exponent `00 01`: first nine bits all zero) |
| V-F17 | `09 05 83 02 00 80 01` | BER | valid: exponent 128, N=1 | constructed by hand from §8.5.7.4 d) (first nine bits `000000001`, not all equal); DER `NonCanonical` since 128 fits in 2 octets (form 01) |
| V-F18 | `09 04 02 31 2E 35` | BER / DER | valid decimal NR2 "1.5" / `NonCanonical` | constructed by hand from §8.5.8 ¶2 (bits 6-1 = `000010` NR2) and §11.3.2.1 (NR3 required) |
| V-F19 | `09 07 03 31 35 2E 45 2D 31` | BER, DER | valid decimal NR3 "15.E-1" = 1.5 | constructed by hand from §8.5.8 ¶2 (NR3 = `000011`) and §11.3.2.4-§11.3.2.6 (mantissa 15 has no zero end digit; `.` then `E`; exponent -1 without plus or leading zero) |
| V-F20 | `09 04 04 31 2E 35` | BER, DER | `RealReserved` | constructed by hand from §8.5.8 ¶3 (bits 6-1 = `000100` reserved) |
| V-F21 | `29 00` | BER, DER | `WrongForm` | constructed by hand from §8.5.1 ¶1 |

### 2.8 OID-IRI and RELATIVE-OID-IRI

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V-U01 | `1F 23 06 2F 32 2F 39 39 39` | BER, DER | OID-IRI "/2/999" | constructed by hand from §8.21.2 ¶1 (UTF-8 of `/2/999`: 2F 32 2F 39 39 39) and §8.1.2.4 (tag 35 = `1F 23`) |
| V-U02 | `1F 24 03 34 2F 33` | BER, DER | RELATIVE-OID-IRI "4/3" | constructed by hand from §8.22.2 ¶1 (first arc has no leading slash per X.680 §35.3) and tag 36 = `1F 24` |
| V-U03 | `1F 23 03 2F C0 AF` | BER, DER | `InvalidUtf8` (overlong `/`) | constructed by hand from §8.21.2 ¶2 (smallest number of octets; C0 AF is the two-octet form of 2F) |
| V-U04 | `1F 23 00` | BER, DER | accept or reject: grammar requires at least `/` + one arc (X.680 §34.3 ¶3). v1 decision: reject as not a value of the notation, but this is a derivation | constructed by hand from X.680 §34.3 |
| V-U05 | `3F 23 00` | BER, DER | `WrongForm` | constructed by hand from §8.21.1 ¶1 |

Vector count: 9 (BOOL) + 28 (INTEGER/ENUMERATED: 23 + 5) + 5 (NULL) + 24 (BIT STRING) + 8 (OCTET STRING) + 28 (OID: 22 + ROID 6) + 21 (REAL) + 5 (IRI) = 128.

## 3. Differences between X.690 (11/2008) and X.690 (02/2021) for these clauses

Method: `itu_compare` on 8.2.2, 8.3.2, 8.5.7.4, 8.5.8, 8.6.2, 8.6.2.4, 8.7.3, 8.19.2, 8.19.4, 8.20.2, 8.21.2, 10.2, 11.2.2, 11.3.1 (all identical), then a word-level diff of the whole text of §8.2 to §8.22 (pdftotext) and §10 to §11.3 after stripping page headers.

1. No difference changes an encoding or a validity rule in §8.2-§8.8, §8.19-§8.22, §10.1-§10.2 or §11.1-§11.3.
2. §8.19.5 example: 2008 uses {2 100 3}, first subidentifier 180, encoding `06 03 81 34 03`; 2021 uses {2 999 3}, first subidentifier 1079, encoding `06 03 88 37 03`. The rule is unchanged. The 2008 example is the only place the standard shows a first arc of 2 with second arc above 39 and a two-octet first subidentifier; both are in the vectors (V-D01, V-D02).
3. Editorial only: "implementor" became "implementer"; "a" became "an" in one sentence; page header and line-wrap changes; 2008 uses "ITU-T Rec. X.680", 2021 uses "Rec. ITU-T X.680".
4. §8.21 and §8.22 (OID-IRI, relative OID-IRI) are present in the 2008-11 PDF in the corpus with identical text. The corpus PDF therefore appears to include that edition's later content; I did not investigate whether it was consolidated (not verified).
5. X.680 2008 vs 2021 was not compared (no 2008 X.680 in the corpus). The 2015 X.690 edition is in the corpus and was not compared.

## 4. Statements not found in X.690 (kept out of the obligation table as rules)

1. Any maximum for an INTEGER, a subidentifier, the number of arcs, or a length (see INT5, OID8): the standard states no limit. X.680 §3.8.48 ¶3 only says encoding-rule limits are chosen not to affect users.
2. The rule "first arc is 0, 1 or 2; if first arc < 2 the second is <= 39" as a stated shall. X.690 §8.19.4 ¶3 says it only in a NOTE; X.660 is the source and is not in the corpus. The decoder split in OID7a is derived from the §8.19.4 formula plus that NOTE.
3. "At least two arcs for an OBJECT IDENTIFIER": only as an X.680 §32.11 NOTE citing X.660. Hence OID9 is derived.
4. A rule on the values of the unused bits in BER (BIT6): none found; only DER (§11.2.1) constrains them.
5. A statement that a BER receiver must reject a non-minimal INTEGER or an OID subidentifier starting with `80`: the clauses use "shall" for the sender-side minimal form (§8.3.2, §8.19.2 ¶6) with no sender's option, so I treat these as invalid in BER. Real-world lenient decoders accept them; the standard gives no such permission.
6. A statement that a decoder rejects indefinite length on a primitive encoding: only the sender rule §8.1.3.2 ¶1 a); the reject is derived.
7. Empty contents for OBJECT IDENTIFIER, RELATIVE-OID, OID-IRI: not addressed in X.690; derived from X.680 notation productions.
8. Binary REAL with N having no octets (no mantissa octets after the exponent): not stated; and REAL form 11 with X = 1 where "first nine bits" cannot exist.
9. How BER decoders treat unused pad bits when comparing bit strings (equality): not stated.
10. The grammar of IRI labels (integerUnicodeLabel, non-integerUnicodeLabel): X.660 and ISO/IEC 10646, not in the corpus. UTF-8 well-formedness details beyond "smallest number of octets" (surrogates, code point range, BOM) rest on ISO/IEC 10646 Annex D, not available.
11. ISO 6093 NR1/NR2/NR3 grammar (REAL decimal): ISO 6093 is not in the corpus; V-F18 and V-F19 rely only on what §8.5.8 and §11.3.2.x say, plus my reading of NR2 as "digits with a decimal mark" and NR3 as "mantissa, `E`, exponent" (not verified).
12. Nesting depth limits for constructed strings: none stated.
13. Behaviour of `BIT STRING` decoders for segment `unused bits` when a nested non-final segment is itself constructed: only implied by the recursion in §8.6.4.1 (BIT12).

## 5. Scope decision: REAL in a first version

Decision: the first version treats REAL as a primitive whose contents are carried opaquely and losslessly, with a small classifier and structural validator, and with no conversion to floating point and no decimal parsing.

Required behaviour for a REAL the crate does not interpret:
1. Accept tag `09` primitive with any definite length, including 0 (+0, §8.5.2); reject `29` and indefinite length (REAL1, G2).
2. Return the contents octets unchanged; BER re-encoding must be byte-for-byte identical (the standard offers many equivalent forms: bases 2/8/16, F, exponent width, mantissa padding, NR1/NR2/NR3, REAL12/REAL14), so normalising would change data.
3. Classify by the first octet (REAL4): Zero (empty), Special (`40`-`43`), Binary, Decimal (NR1-NR3), and reject reserved values (REAL5, REAL8, REAL14) and impossible lengths (REAL10). This is cheap, needs no outside standard, and is verifiable against §8.5.
4. In DER mode, run the §11.3 checks that can be done on the octets (REAL13; NR3 only and the syntax rules of REAL15), which need no arithmetic.

Justification:
1. X.680 §21.3 ¶3 makes base-2 and base-10 values distinct abstract values, and §21.3 ¶1 includes -0, +-INF and NaN; an f64 cannot round-trip those distinctions, so any v1 conversion would be lossy by design.
2. Binary values have an unbounded exponent (§8.5.7.4 d) and an arbitrary-length mantissa; exact conversion needs a bignum or explicit rounding policy that the standard does not specify.
3. Decimal values depend on ISO 6093, which is outside the corpus and unverified here.
4. Real-world protocols this crate is aimed at use REAL rarely; opaque carriage costs nothing in correctness.
Deferred to a later version: `to_f64`/`from_f64` for binary (with documented rounding), decimal parsing, and base-conversion helpers.

## 6. Hazards for the implementer

1. Do not follow the suggestion that `FF 7F` is a non-minimal INTEGER: it is -129 and minimal. Non-minimal two-octet forms are `00 xx` with xx < 0x80 and `FF xx` with xx >= 0x80 (V-I16 to V-I19). `00 80` is minimal (128).
2. §8.3.2 and §8.19.2 ¶6 minimality rules bind BER as well; only DER adds length minimality and the canonical choices.
3. Tags 35 and 36 need `1F 23` and `1F 24`; a one-octet `23`/`24` is the constructed BIT STRING/OCTET STRING tag, not OID-IRI.
4. In DER, constructed strings are rejected outright (§10.2) but in BER the segments may be nested and may be empty; non-final segments of a bit string must be whole octets.
5. DER bit strings: the trailing-zero strip (§11.2.2) applies only to NamedBitList types, which is schema knowledge, not visible in the TLV. Pad bits being zero (§11.2.1) applies always.
6. OID arcs are unbounded; the first subidentifier is `40*X + Y` with Y unbounded when X = 2.
7. NULL and BOOLEAN length checks must come before reading contents, and a long-form length like `81 00` is legal BER.
8. REAL: keep it opaque in v1 (section 5).
