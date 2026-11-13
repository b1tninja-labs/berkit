# X.690 structured types and tagging: obligations, vectors, boundary

Source: Rec. ITU-T X.690 (02/2021) | ISO/IEC 8825-1:2021 (cited "X.690 §n ¶k"; ¶k is the k-th sentence of the
itulib extraction) and Rec. ITU-T X.680 (02/2021) ("X.680 §n"). Erratum 1 (09/2021) only replaces Figure 4
(high-tag-number identifier octets); it touches none of the clauses below. All rules were read through the
itulib MCP tools. Scratch text extracts are `x690.txt`, `x690-2008.txt`, `x680.txt` in this directory.

Error classes used below:
`Truncated` (input ends early) | `Form` (primitive/constructed bit wrong for the type) | `Length` (length octets
illegal, or a child overruns its parent) | `NonMinimalLength` | `UnexpectedEoc` | `MissingEoc` |
`IndefiniteInDer` | `SetOrder` (components not in canonical tag order) | `SetOfOrder` (elements not ascending) |
`DefaultPresent` | `TagMismatch` (schema expected another tag) | `Schema` (schema-level violation).

## 1. Obligations

"Enc" = what an encoder must do. "BER" / "DER" = what a decoder does in that mode. For DER the standard states
restrictions on the *encoding* (X.690 §10, §11); it states no decoder duty to reject. "reject in DER" below is
therefore an inference from X.690 §7.1 (dynamic conformance is clauses 8 to 12) plus the §10/§11 "shall";
see section 4. "Schema?" = needs knowledge of the ASN.1 type.

| ID | Clause and sentence | Rule | Enc | BER decode | DER decode | Schema? | Error class |
|---|---|---|---|---|---|---|---|
| S1 | X.690 §8.9.1 ¶1 | SEQUENCE encoding is constructed (so identifier bit 6 = 1 per §8.1.2.5 ¶1) | emit 0x30 | reject 0x10 | reject 0x10 | No for universal 16; yes for a tagged field | Form |
| S2 | §8.9.2 ¶1 | Contents = one complete encoding per component, in definition order, except OPTIONAL/DEFAULT ones | write in declared order | match positionally | same | Yes | Schema |
| S3 | §8.9.3 ¶1-2 | OPTIONAL/DEFAULT component may be present or absent; if present it sits at its declared position | omit or place | accept both | accept (but see D1) | Yes | Schema |
| S4 | X.680 §25.6.1 ¶1 | Consecutive OPTIONAL/DEFAULT components and the next component need distinct tags (makes positional decoding unambiguous) | n/a | n/a | n/a | Yes (schema validity) | Schema |
| SO1 | §8.10.1 ¶1 | SEQUENCE OF is constructed | 0x30 | reject 0x10 | same | No | Form |
| SO2 | §8.10.2 ¶1 | Contents = zero, one or more complete encodings of the element type; empty is legal | `30 00` | accept | accept | No (count); element type needs schema | Schema |
| SO3 | §8.10.3 ¶1 | Element order = order of the value; order is preserved | keep order | keep | keep | No | none |
| T1 | §8.11.1 ¶1 | SET is constructed | 0x31 | reject 0x11 | same | No | Form |
| T2 | §8.11.2 ¶1 | SET contents: one encoding per component, in an order chosen by the sender (BER), unless OPTIONAL/DEFAULT | any order | accept any order | see T4 | Yes | Schema |
| T3 | §8.11.3 ¶1-2 | OPTIONAL/DEFAULT may be absent; order of SET values carries no meaning | | | | Yes | Schema |
| T4 | §10.3 ¶1 (DER); §9.3 ¶1 (CER) | Encodings of SET components appear in the canonical tag order of X.680 §8.6 | sort by tag | no check | reject if out of order | No, if the caller says "this is a SET" | SetOrder |
| T5 | X.680 §8.6 ¶1 a) | Canonical order: universal, then application, then context-specific, then private | | | | No | SetOrder |
| T6 | X.680 §8.6 ¶1 b) | Within a class, ascending tag number (a number, so [31] and [200] compare as numbers, not as identifier bytes) | | | | No | SetOrder |
| T7 | §10.3 ¶2 (NOTE) vs §9.3 ¶2 | DER: an untagged CHOICE component sorts by the tag of the alternative actually encoded. CER: it sorts as if it had the smallest tag anywhere inside it. The two rules differ | DER: use the actual outermost tag | n/a | the actual outermost tag is all the check needs | DER no; CER yes | SetOrder |
| T8 | X.680 §27.3 ¶1 | SET component tags are all different | n/a | n/a | n/a | Yes | Schema |
| TO1 | §8.12.1 ¶1 | SET OF is constructed | 0x31 | reject 0x11 | same | No | Form |
| TO2 | §8.12.2 ¶1 | Contents as for SEQUENCE OF (§8.10.2) | | | | | |
| TO3 | §8.12.3 ¶1 | Order of values need not survive encode then decode | any order | do not promise to preserve | | No | none |
| TO4 | §11.6 ¶1 | Encodings of SET OF elements appear in ascending order; compare the complete element encodings (identifier octets first) as octet strings, padding the shorter with trailing 0-octets | sort | no check | reject if descending | No, if the caller says "this is a SET OF" | SetOfOrder |
| TO5 | §11.6 ¶2 | The padding octets exist only for comparison and are never written | | | | No | |
| TO6 | not found in X.690 | Whether equal (duplicate) elements are allowed, i.e. whether "ascending" is strict | | | see section 4 | | |
| C1 | §8.13 ¶1 | A CHOICE value is encoded exactly as the chosen alternative; no wrapper | no extra TLV | cannot be seen | cannot be seen | Yes (which alternatives exist) | TagMismatch |
| C2 | §8.13 ¶3 (NOTE 2); §8.1.2.6 ¶1; X.680 §29.8 ¶1 | The identifier octets carry the tag of the chosen alternative, so the choice has no single tag | | | | Yes | |
| C3 | X.680 §29.3 ¶1 | Alternative tags are distinct | | | | Yes | Schema |
| P1 | §8.14.1 ¶1-2 | TaggedType: rules of §8.14.2 to .4 apply; EncodingPrefixedType uses the encoding of its inner Type | | | | Yes | |
| P2 | §8.14.2 ¶1 | Tagged encoding is derived from the base encoding (the encoding of the inner type) | | | | Yes | |
| P3 | §8.14.3 ¶1 | Not implicit (explicit): encoding is constructed; contents are the complete base encoding (a TLV inside) | A0-style TLV wrapping the base TLV | expect exactly the base TLV inside | same | Yes | Form |
| P4 | §8.14.4 ¶1 a) | Implicit: constructed iff the base encoding is constructed, else primitive | copy bit 6 of the base | | | Yes | Form |
| P5 | §8.14.4 ¶1 b) | Implicit: contents octets equal the base contents octets; only identifier octets change (the base tag is replaced, not nested) | swap identifier | | | Yes | |
| P6 | X.680 §31.2.7 ¶1 a)-c), ¶2 | Explicit if EXPLICIT is written, or the module default is EXPLICIT/empty, or the tagged type is an untagged CHOICE / untagged open type / untagged DummyReference (even under IMPLICIT or AUTOMATIC TAGS). Else implicit | choose per rule | | | Yes | |
| P7 | X.680 §31.2.9 ¶1 | The IMPLICIT keyword must not be applied to an untagged CHOICE or untagged open type | | | | Yes | Schema |
| P8 | X.680 §31.2.4 ¶1 | Empty class means context-specific | | | | Yes | |
| P9 | X.680 §31.2.5 ¶1 | A UNIVERSAL tag may be used only for types defined in X.680/X.690 | | | | Yes | |
| P10 | X.680 §25.10 ¶1 a)-e), ¶3 | Automatic tagging: context-specific tags 0,1,2... in order over the root list, then continuing in the extension additions; implicit unless the type is a CHOICE, open type or DummyReference | compile-time | | | Yes | |
| P11 | X.680 §25.3 ¶1 | Automatic tagging applies to a component list only if none of its components is textually tagged | | | | Yes | |
| P12 | X.680 §25.12 ¶1 | Omitting a DEFAULT component is equivalent to inserting the default value | | | | Yes | |
| O1 | §8.15 ¶1-2 | An open type is encoded as the complete encoding of the (other) type its value belongs to, including its identifier octets | caller supplies a TLV | return the TLV unopened | same | Yes (which type) | |
| O2 | §8.1.2.7 ¶1-2 | The tag of an open type is indeterminate; its encoding is that of the assigned type | | | | Yes | |
| O3 | X.681 §14.2 (b) | An open type cannot be used where a tag distinct from other types is required (so implicit tagging of one is excluded by P7) | | | | Yes | |
| I1 | §8.16.1 ¶1 | INSTANCE OF = BER of `[UNIVERSAL 8] IMPLICIT SEQUENCE { type-id <Class>.&id, value [0] EXPLICIT <Class>.&Type }` | | | | Yes | |
| I2 | §8.16.2 ¶1 | Component values are those of X.681 §C.7 | | | | Yes | |
| E1 | §8.17.1 ¶1; §8.17.2 ¶1 | EMBEDDED PDV = BER of the type in X.680 §36.5; data-value OCTET STRING holds the encoding in the identified transfer syntax | | | | Yes | |
| X1 | §8.18.1 ¶1 | EXTERNAL = BER of `[UNIVERSAL 8] IMPLICIT SEQUENCE { direct-reference OID OPTIONAL, indirect-reference INTEGER OPTIONAL, data-value-descriptor ObjectDescriptor OPTIONAL, encoding CHOICE { single-ASN1-type [0] ABSTRACT-SYNTAX.&Type, octet-aligned [1] IMPLICIT OCTET STRING, arbitrary [2] IMPLICIT BIT STRING } }` in an EXPLICIT TAGS environment | | | | Yes | |
| X2 | §8.18.3 ¶1 | data-value-descriptor present iff present in the abstract value | | | | Yes | Schema |
| X3 | §8.18.4 ¶1, Table 2 | direct-/indirect-reference follow Table 2: syntax -> direct present, indirect absent; presentation-context-id -> direct absent, indirect present; context-negotiation -> both present; syntaxes, transfer-syntax, fixed cannot occur. (Table 2 checked against `pdftotext -raw`; matches) | | | | Yes | Schema |
| X4 | §8.18.6-8.18.8 | Choice of `encoding` alternative: single ASN.1 type -> any of the three is a sender option; whole number of octets -> octet-aligned or arbitrary; not whole octets -> arbitrary only | | accept any | accept any | Yes | Schema |
| X5 | §8.18.9 ¶1, §8.18.10, §8.18.11 | single-ASN1-type: the open type is replaced by the real type; octet-aligned: the encoded octets are the OCTET STRING value; arbitrary: the encoding is the BIT STRING value | | | | Yes | |
| L1 | §8.1.3.2 ¶1 a)-c) | Primitive: definite length. Constructed with all contents available: definite or indefinite (sender's option). Constructed and not all available: indefinite | | | | No | |
| L2 | §8.1.3.6 ¶1, §8.1.5 ¶1-2 | Indefinite length is one octet (0x80) and is closed by end-of-contents octets `00 00`; EOC present iff the length is indefinite | | match EOC to its opener | same, but see L4 | No | MissingEoc / UnexpectedEoc |
| L3 | §8.1.5 ¶3-4 (NOTE) | EOC is a primitive universal tag-0 TLV with zero length | | a `00 00` seen where a child TLV is expected closes the nearest open indefinite TLV | | No | |
| L4 | §9.1 ¶1 (CER); §10.1 ¶1 (DER) | DER: definite length only, minimum number of length octets. CER: constructed must be indefinite; primitive minimal | DER: always definite, minimal | accept all of §8.1.3 | reject 0x80; reject non-minimal long form | No | IndefiniteInDer / NonMinimalLength |
| L5 | §8.1.3.5 ¶2 c) | First long-form length octet 0xFF is forbidden | | reject | reject | No | Length |
| L6 | §8.1.3.5 ¶5 (NOTE 2); §7.3 ¶2 | In BER the sender may use more length octets than needed; BER receivers must accept all alternatives | | accept | (rejected by L4) | No | |
| L7 | §8.1.1.2 ¶1 | EOC octets must not appear unless the length requires them | | reject stray EOC inside a definite-length parent | same | No | UnexpectedEoc |
| L8 | §8.1.2.2 ¶1, §8.1.2.4.1 ¶1, §8.1.2.4.2 ¶1 | Tag number 0..30 in one octet; >= 31 uses 0x1F then base-128 digits, bit 8 set on all but the last, first digit not zero | | | | No | |
| L9 | §8.1.2.2 ¶1 | Tag numbers 0..30 must use the one-octet form (so `9F 1E` is not a legal encoding of [30]) | | | | No | |
| D1 | §11.5 ¶1 | DER and CER: a SET/SEQUENCE component equal to its DEFAULT must not be encoded | omit | accept (value equals default) | reject presence of a default-equal component | Yes | DefaultPresent |
| D2 | §10.2 ¶1 | DER: no constructed form for bit strings, octet strings, restricted strings (out of this slice, but a TLV crate meets it when walking children) | | accept | reject | No (universal tags 3, 4, 12, 18-22, 25-30) | Form |
| M1 | §8.1.1.4 ¶1-2 | Constraints and extension markers do not change encodings; extension additions are encoded as if in the root | | | | Yes | |
| M2 | §7.3 ¶2 | BER receiver supports every permitted alternative | | | | | |

CER and DER have no further rule for SEQUENCE OF, CHOICE, tagged types, open types, INSTANCE OF, EMBEDDED PDV or
EXTERNAL: clauses 9, 10 and 11 contain only the rules listed (checked by reading the full outline of §9, §10, §11;
§11 has 11.1-11.9 and none concerns these types).

## 2. Test vectors

Leaf encodings used: INTEGER 5 = `02 01 05` (two's-complement contents, X.690 §8.3.3 ¶1; tag 02 from X.680 §8.6 Table 1);
BOOLEAN TRUE in DER = `01 01 FF` (X.690 §11.1 ¶1). Provenance wording is as required.

### 2.1 Taken from X.690

| hex input | mode | expected | provenance |
|---|---|---|---|
| `30 0A 16 05 53 6D 69 74 68 01 01 FF` | BER, DER | SEQUENCE { IA5String "Smith", BOOLEAN TRUE }, valid in both | from X.690 §8.9.3 example (the standard prints `30 0A`, `16 05`, "Smith", `01 01 FF`; the octets of "Smith" are my ASCII expansion; 7 + 3 = 10 agrees with the printed length) |
| `1A 05 4A 6F 6E 65 73` | BER, DER | VisibleString "Jones" | from X.690 §8.14.4 example, Type1 |
| `43 05 4A 6F 6E 65 73` | BER, DER | [APPLICATION 3] IMPLICIT VisibleString "Jones" | from X.690 §8.14.4 example, Type2 |
| `A2 07 43 05 4A 6F 6E 65 73` | BER, DER | Type3 ::= [2] Type2: explicit [2] wrapping the [APPLICATION 3] TLV | from X.690 §8.14.4 example, Type3 |
| `67 07 43 05 4A 6F 6E 65 73` | BER, DER | Type4 ::= [APPLICATION 7] IMPLICIT Type3: constructed, the inner [APPLICATION 3] TLV kept, the [2] tag gone | from X.690 §8.14.4 example, Type4 |
| `82 05 4A 6F 6E 65 73` | BER, DER | Type5 ::= [2] IMPLICIT Type2: primitive, tag [2] replaces [APPLICATION 3] | from X.690 §8.14.4 example, Type5 |
| `31 1F 61 11 1A 05 52 61 6C 70 68 1A 01 54 1A 05 53 6D 69 74 68 A0 0A 43 08 31 39 35 37 31 31 31 31` | BER, DER | ChildInformation SET { name (APPLICATION 1) , dateOfBirth [0] }: components already in canonical order (application before context-specific), valid in both | from X.690 Annex A.3 (the first child of the record; `Set 31 1F ...`), schema from Annex A.1. Hex re-assembled by hand from the annex text (1F = 2+17+2+10 = 31); the annex is informative |
| `A3 42 31 1F ... 31 1F ...` | BER | `children [3] IMPLICIT SEQUENCE OF ChildInformation`: implicit tag keeps constructed bit; two child SETs | from X.690 Annex A.3 (`A3 42 Set`); 0x42 = 2 x 33 |
| `60 81 85 ...` | BER, DER (header only) | identifier 0x60 = [APPLICATION 0] constructed (PersonnelRecord), length `81 85` = 133, long form with one length octet; minimal, since 133 > 127 | from X.690 Annex A.3 (`60 8185`) |
| `43 08 31 39 35 37 31 31 31 31` | BER, DER | Date ::= [APPLICATION 3] IMPLICIT VisibleString "19571111" | from X.690 Annex A.3 |

Only the opening of the PersonnelRecord was transcribed; the full record is in Annex A.3. The "Smith" row is
not a SET: it is a SEQUENCE of two components, so no ordering rule applies to it.

### 2.2 Constructed by hand

Notation: every vector is a complete TLV; spaces only for readability.

**Empty and simple SEQUENCE / SET**

| hex input | mode | expected | provenance |
|---|---|---|---|
| `30 00` | BER, DER | empty SEQUENCE (valid: zero components, all OPTIONAL) | constructed by hand from §8.9.1 ¶1 (0x30 = universal, constructed, 16) and §8.1.3.4 ¶1 (length 0) |
| `31 00` | BER, DER | empty SET / empty SET OF | constructed by hand from §8.11.1 ¶1 and §8.12.2 ¶1 |
| `10 00` | BER, DER | Form error: SEQUENCE must be constructed | constructed by hand from §8.9.1 ¶1; 0x10 = universal 16 with bit 6 = 0 (§8.1.2.5 ¶1) |
| `30 03 02 01 05` | BER, DER | SEQUENCE { INTEGER 5 } | constructed by hand from §8.9.2 ¶1; length 3 = one TLV of 3 octets |
| `30 03 02 02 05` | BER, DER | Length error: child (needs 2 content octets) overruns parent (1 left) | constructed by hand from §8.1.3.4 ¶1 |
| `30 03 02 01` | BER, DER | Truncated | constructed by hand: parent declares 3 octets, only 2 present |

**Indefinite length**

| hex input | mode | expected | provenance |
|---|---|---|---|
| `30 80 02 01 05 00 00` | BER | SEQUENCE { INTEGER 5 } | constructed by hand from §8.1.3.6 ¶1 (length octet 0x80) and §8.1.5 ¶2 (EOC = `00 00`) |
| `30 80 02 01 05 00 00` | DER | IndefiniteInDer | constructed by hand from §10.1 ¶1 |
| `30 80 00 00` | BER | empty SEQUENCE, indefinite form | constructed by hand from §8.1.3.6 and §8.1.5 |
| `30 80 30 80 02 01 05 00 00 00 00` | BER | SEQUENCE { SEQUENCE { INTEGER 5 } }, both indefinite; the first `00 00` closes the inner, the second the outer | constructed by hand from §8.1.5 ¶1-2 and §8.1.1.1 ¶1 d) |
| `30 80 30 03 02 01 05 00 00` | BER | outer indefinite, inner definite; the single `00 00` closes the outer | constructed by hand from §8.1.1.2 ¶1 (inner has no EOC) |
| `30 80 02 01 05` | BER | MissingEoc (input ends while an indefinite TLV is open) | constructed by hand from §8.1.5 ¶1 |
| `30 80 02 01 05 00` | BER | MissingEoc / Truncated (a single `00` is not an EOC) | constructed by hand from §8.1.5 ¶2 |
| `30 80 30 80 02 01 05 00 00` | BER | MissingEoc (inner closed, outer not) | constructed by hand from §8.1.5 ¶1 |
| `30 05 02 01 05 00 00` | BER, DER | UnexpectedEoc (an EOC, i.e. tag 0 length 0, inside a definite-length parent) | constructed by hand from §8.1.1.2 ¶1 and §8.1.5 ¶1 |
| `00 00` | BER, DER | UnexpectedEoc at top level | constructed by hand from §8.1.5 ¶1 |
| `02 80 05 00 00` | BER, DER | Length error: indefinite length on a primitive (INTEGER) is not allowed | constructed by hand from §8.1.3.2 ¶1 a) and §8.3.1 (INTEGER is primitive). The standard states this only as a sender rule, see section 4 |
| `30 FF ...` | BER, DER | Length error | constructed by hand from §8.1.3.5 ¶2 c) |
| `30 81 03 02 01 05` | BER | SEQUENCE { INTEGER 5 }, length in a longer form than necessary | constructed by hand from §8.1.3.5 ¶5 (NOTE 2) |
| `30 81 03 02 01 05` | DER | NonMinimalLength | constructed by hand from §10.1 ¶1 |

**SET order by tag** (all leaves have 1 content octet, so each component is 3 octets; contents length 12 = 0x0C)

| hex input | mode | expected | provenance |
|---|---|---|---|
| `31 0C 02 01 05 41 01 05 80 01 05 C0 01 05` | BER, DER | accept: universal 2, [APPLICATION 1], [0], [PRIVATE 0] in canonical order | constructed by hand from X.680 §8.6 ¶1 a) (class order) and §10.3 ¶1; identifier octets 02, 41 (class bits 01), 80 (10), C0 (11) from §8.1.2.2 ¶2 Table 1 |
| `31 0C C0 01 05 80 01 05 41 01 05 02 01 05` | DER | SetOrder (exact reverse) | same construction, reversed |
| `31 0C C0 01 05 80 01 05 41 01 05 02 01 05` | BER | accept (sender's order is free) | §8.11.2 ¶1 |
| `31 0C 80 01 05 41 01 05 02 01 05 C0 01 05` | DER | SetOrder (context-specific before universal) | same construction |
| `31 08 A0 03 02 01 05 81 01 05` | DER | accept: [0] EXPLICIT INTEGER (constructed, A0) before [1] IMPLICIT INTEGER (primitive, 81) | constructed by hand from X.680 §8.6 ¶1 b) and §8.14.3 ¶1 (explicit => constructed, bit 6 set, A0 = 0x80+0x20+0) |
| `31 08 81 01 05 A0 03 02 01 05` | DER | SetOrder. Sorting by raw identifier byte would wrongly accept this (0x81 < 0xA0); the order is by class and tag number only | same construction, reversed; hazard H3 |
| `31 0C 82 01 05 9F 1F 01 05 9F 81 48 01 05` | DER | accept: [2], [31], [200] ascending by number (200 = 0x01 and 0x48 in base 128 -> `81 48`) | constructed by hand from §8.1.2.4.1 ¶1 (leading octet 0x9F) and §8.1.2.4.2 ¶1 (digits, bit 8 on all but the last) |
| `31 0C 9F 81 48 01 05 9F 1F 01 05 82 01 05` | DER | SetOrder | same, reversed |
| `31 0B A1 03 82 01 02 83 01 01 86 01 03` | DER | accept under the §10.3 reading (see T7): schema `A ::= SET { a [3] INTEGER, b [1] CHOICE { c [2] INTEGER, d [4] INTEGER }, e CHOICE { f CHOICE { g [5] INTEGER, h [6] INTEGER }, i CHOICE { j [0] INTEGER } } }` in an IMPLICIT TAGS environment, value b = c 2, a = 1, e = h 3. Encoded as b ([1] explicit since b is tagged CHOICE, X.680 §31.2.7 c: `A1 03 82 01 02`), a (`83 01 01`), e (`86 01 03`); order by actual outer tag 1, 3, 6 | constructed by hand from X.690 §9.3 example schema, §10.3 ¶1-2 |
| `31 0B 86 01 03 A1 03 82 01 02 83 01 01` | CER (not DER) | the order the §9.3 rule gives for the same value: e first (smallest tag inside e is [0]), then b, then a. Under DER this is SetOrder (actual tag 6 before 1). Included to show the two rules disagree | constructed by hand from §9.3 ¶2 and the §9.3 example result "e, b, a" |

**SET OF order** (the encodings are compared whole, identifier octets first, §11.6 ¶1)

| hex input | mode | expected | provenance |
|---|---|---|---|
| `31 06 02 01 01 02 01 02` | BER, DER | accept: SET OF INTEGER {1, 2}; encodings `02 01 01` < `02 01 02` | constructed by hand from §11.6 ¶1 |
| `31 06 02 01 02 02 01 01` | DER | SetOfOrder | same, reversed |
| `31 06 02 01 02 02 01 01` | BER | accept (order is the sender's, §8.12.3 ¶1) | §8.12.3 ¶1 |
| `31 06 02 01 05 02 01 05` | BER | accept: two equal elements | see TO6; X.690 has no rule against duplicates |
| `31 06 02 01 05 02 01 05` | DER | **decision needed** (TO6). Recommended: accept (non-decreasing); the equal encodings compare equal and no clause forbids equal elements. Test it as "accept" and record the decision | constructed by hand from §11.6 ¶1; the sentence says "ascending" only |
| `31 07 02 02 00 80 04 01 41` | DER | accept: SET OF CHOICE { i INTEGER, o OCTET STRING }; elements `02 02 00 80` (4 octets) and `04 01 41` (3 octets). The 4-octet one sorts first because 0x02 < 0x04. A "shorter first" rule would wrongly reject this | constructed by hand from §11.6 ¶1 (INTEGER 128 = `00 80` by §8.3.2) |
| `31 07 04 01 41 02 02 00 80` | DER | SetOfOrder | same, reversed |
| `31 0A 02 02 00 80 02 02 01 00 02 01 7F` | DER | SetOfOrder: same tag, the element with the larger length octet (02) must follow the smaller (01) | constructed by hand from §11.6 ¶1 |
| `31 0A 02 01 7F 02 02 00 80 02 02 01 00` | DER | accept (`02 01 7F` < `02 02 00 80` < `02 02 01 00`) | same |

Padding rule. The comparison, not any TLV, is what tests the padding sentence (§11.6 ¶1-2). Function-level
vectors for `compare(a, b)`; they are not well-formed TLVs:

| a | b | result | provenance |
|---|---|---|---|
| `01` | `01 00` | Equal (padding makes `01` into `01 00`) | constructed by hand from §11.6 ¶1 |
| `01 00` | `01` | Equal | same |
| `01` | `01 01` | Less (`01 00` vs `01 01`) | same |
| `01 FF` | `02` | Less (`02 00` is larger than `01 FF`) | same |
| `02` | `01 FF` | Greater | same |
| `01 00 00` | `01` | Equal | same |

Observation (derived by me, not stated in X.690): for two complete definite-length TLVs a proper prefix
cannot occur. If A is a proper prefix of B, B's identifier and length octets equal A's, so B's contents length
equals A's, so B has the same total length as A. The padding therefore never changes the order of well-formed
definite-length encodings; it matters only for sloppy inputs. A plain lexicographic compare of whole encodings
(shorter-is-smaller on a tied prefix) gives the same order on well-formed input. What does matter is comparing
whole encodings (tag first), not contents only and not length first (the CHOICE vectors above).

**Implicit versus explicit tagging of the same INTEGER** (INTEGER 5, tag [2], hand-built as the §8.14.4 example)

| hex input | mode | expected | provenance |
|---|---|---|---|
| `A2 03 02 01 05` | BER, DER | `[2] EXPLICIT INTEGER` 5: constructed, contains the full INTEGER TLV | constructed by hand from §8.14.3 ¶1 (the Type3 pattern in §8.14.4: `A2 07 43 05 ...`) |
| `82 01 05` | BER, DER | `[2] IMPLICIT INTEGER` 5: primitive, INTEGER tag replaced | constructed by hand from §8.14.4 ¶1 (the Type5 pattern: `82 05 ...`) |
| `82 03 02 01 05` | BER, DER | TagMismatch/Form if the schema says IMPLICIT INTEGER: 0x82 is primitive, content 3 octets are the integer `02 01 05` = 0x020105; decoded as an INTEGER it is the number 131333. It is not an error at the TLV level | constructed by hand from §8.14.4 ¶1 b); shows a TLV reader cannot tell the two forms apart |
| `A2 03 02 01 05` | BER, DER | Form error if the schema says IMPLICIT INTEGER: constructed bit set on an INTEGER's tag | constructed by hand from §8.14.4 ¶1 a) |
| `82 01 05` | BER, DER | Form error if the schema says EXPLICIT: explicit requires constructed | constructed by hand from §8.14.3 ¶1 |
| `A2 03 04 01 05` | BER, DER | TagMismatch if the schema says `[2] EXPLICIT INTEGER` (inner tag 04) | constructed by hand from §8.14.3 ¶1 |

**Tagged CHOICE** (schema `C ::= CHOICE { i INTEGER, b BOOLEAN }`)

| hex input | mode | expected | provenance |
|---|---|---|---|
| `02 01 05` | BER, DER | untagged C = i 5 (no wrapper) | constructed by hand from §8.13 ¶1 |
| `01 01 FF` | BER, DER | untagged C = b TRUE | same, with §11.1 ¶1 for DER |
| `A1 03 02 01 05` | BER, DER | `T ::= [1] C` = i 5 under EXPLICIT TAGS | constructed by hand from §8.14.3 ¶1 and X.680 §31.2.7 ¶1 a)/b) |
| `A1 03 02 01 05` | BER, DER | the same bytes when the module says IMPLICIT TAGS or AUTOMATIC TAGS: tagging a CHOICE is still explicit | constructed by hand from X.680 §31.2.7 ¶1 c) |
| `81 01 05` | BER, DER | Form/Schema error for `[1] C` under any tagging: implicit tagging of an untagged CHOICE is never used | constructed by hand from X.680 §31.2.7 ¶1 c), §31.2.9 ¶1 |
| `A1 03 01 01 FF` | BER, DER | `[1] C` = b TRUE | constructed by hand, as above |

**DEFAULT omission** (schema `D ::= SEQUENCE { a INTEGER DEFAULT 5 }`, and `E ::= SEQUENCE { a INTEGER DEFAULT 5, b INTEGER OPTIONAL }`, no tags: both fields are universal 2, so `E` is ambiguous per X.680 §25.6.1 ¶1; use `[0]`/`[1]` implicit tags for E)

| hex input | mode | expected | provenance |
|---|---|---|---|
| `30 00` | BER, DER | D with a = 5 (default) | constructed by hand from §8.9.3 ¶1 and X.680 §25.12 ¶1 |
| `30 03 02 01 05` | BER | D with a = 5, explicit | §8.9.3 ¶1 |
| `30 03 02 01 05` | DER | DefaultPresent | §11.5 ¶1 |
| `30 03 02 01 06` | BER, DER | D with a = 6 | §8.9.3 ¶1 |
| `30 03 80 01 05` | DER | DefaultPresent for `E ::= SEQUENCE { a [0] IMPLICIT INTEGER DEFAULT 5, b [1] IMPLICIT INTEGER OPTIONAL }` with a = 5 | §11.5 ¶1 |

**Open type, INSTANCE OF, EXTERNAL**

| hex input | mode | expected | provenance |
|---|---|---|---|
| `28 08 06 01 2A A0 03 02 01 05` | BER, DER | INSTANCE OF: `[UNIVERSAL 8] IMPLICIT SEQUENCE` is constructed, so identifier 0x28. Contents: type-id as an OBJECT IDENTIFIER TLV `06 01 2A` (the content byte is opaque here), then `value [0] EXPLICIT` = `A0 03` wrapping the open-type value, a complete INTEGER TLV `02 01 05`. Length 3 + 5 = 8 | constructed by hand from §8.16.1 ¶1, §8.14.4 ¶1 a), §8.14.3 ¶1 and §8.15 ¶2 |
| `28 08 06 01 2A A0 03 02 01 05` | BER, DER | the same bytes are also an EXTERNAL with direct-reference and `single-ASN1-type [0]` (explicit, so `A0`) | from §8.16.1 ¶2 (NOTE: the INSTANCE OF encoding equals the EXTERNAL syntax alternative) |
| `28 08 06 01 2A A0 03 02 01 05` | BER | at the TLV level: a constructed universal-8 node whose children are universal 6 and constructed context [0]; the open-type value is the single child of the `A0` node and stays unopened | §8.15 ¶2 |

The OBJECT IDENTIFIER content rules belong to another slice.

## 3. Differences between X.690 (11/2008) and (02/2021)

`itu_compare` was run, sentence by sentence, on §8.1.5, §8.1.3.2, §8.1.3.5, §8.1.2.4.2, §8.6, §8.9 and its
subclauses 8.9.2 and 8.9.3, §8.10 and 8.10.3, §8.11 and 8.11.2 and 8.11.3, §8.12, §8.13, §8.14 and 8.14.1,
8.14.3, 8.14.4, §8.15, §8.16 and 8.16.1, §8.17 and 8.17.1 and 8.17.2, §8.18 and 8.18.1, 8.18.4, 8.18.9, §9.1,
§9.3, §10.1, §10.3, §11.5, §11.6. Result: **no sentence in any of them changed in a way that alters an encoding
or a validity rule.** The only reported change is §8.1.5 ¶1, where the 2008 text carries the clause heading
"End-of-contents octets" run into the sentence (a typesetting artefact of the extraction, not a change of
meaning). `itu_compare` on §8.1.3 returned "not in 2008" because the clause is a heading with subclauses in 2008
(numbering differs); the leaf clauses 8.1.3.2 and 8.1.3.5 are identical.
Not compared: the 2008 edition of X.680 (not registered in itulib: only X.680 02/2021 is), so the X.680 rules
cited here (§8.6 canonical order, §31.2.7, §31.2.9, §25.10, §27.3, §29.3) are known only in the 2021 text.
The itulib note says the 2008 file is the base text without Corrigenda 1 (10/2011) and 2 (03/2014); those
corrigenda were not read.

## 4. Statements not found in X.690

- A rule that a DER *decoder* must reject a non-canonical encoding. §10 and §11 constrain "the encoding"; §7.1
  says dynamic conformance is specified by clauses 8 to 12; §7.3 ¶2 says BER receivers accept all alternatives.
  "Reject in DER" in the table is my inference.
- A rule that a decoder must reject indefinite length on a primitive encoding. §8.1.3.2 ¶1 a) is a sender rule
  (primitive uses the definite form). Rejection is an inference.
- Any ban or allowance of equal elements in a SET OF (TO6). §11.6 says "ascending". A search of X.680 for
  duplicate elements in a set-of value found nothing relevant. Strict versus non-strict is undecided by the text.
- A tie-break for equal tags in a SET (none exists: X.680 §27.3 requires distinct tags, so ties cannot occur in
  a valid schema).
- Any limit on nesting depth, on the number of components, or on length values (the extract contains only
  "maximum" in an unrelated REAL annex). Limits are an implementation matter.
- Any statement of what a decoder does with a tag number of 31 or more encoded in the long form when it fits in
  one octet, beyond the "shall comprise a single octet" of §8.1.2.2 ¶1 (the rule is there; no wording about
  rejection).
- A normative statement for the DER ordering of an untagged CHOICE: only the NOTE of §10.3 ¶2 (a note is not a
  requirement; the requirement is ¶1 together with X.680 §29.8 ¶1, under which the tag depends on the chosen
  alternative).
- X.680's 2008 text (see section 3).

## 5. TLV-level versus schema-level

A "TLV-level crate" parses `identifier / length / contents / EOC` into a tree and writes it back.

### 5.1 Enforceable at the TLV level (needs no schema)

| Rule | Clause | Note |
|---|---|---|
| Identifier octets: class bits, constructed bit, high-tag-number form, no leading zero digit, numbers 0..30 in one octet | §8.1.2.2, §8.1.2.4.1, §8.1.2.4.2, §8.1.2.5 | |
| Length octets: short form, long form, 0xFF forbidden, definite versus indefinite | §8.1.3.1 to .6 | |
| Child overruns parent; truncation | §8.1.3 | |
| EOC matching: `00 00` closes the nearest open indefinite TLV; stray EOC in a definite parent is an error; missing EOC is an error | §8.1.5 ¶1-2, §8.1.1.2 | EOC is never a node in the tree |
| Indefinite only on constructed encodings (decoder inference) | §8.1.3.2 ¶1 a) | |
| Universal SEQUENCE/SET (tags 16, 17) must be constructed | §8.9.1, §8.10.1, §8.11.1, §8.12.1 | needs only the tag; for any other universal type the form is fixed too (other slices) |
| DER: definite length only, minimal length octets | §10.1 | |
| DER: no constructed strings (tags 3, 4, 12, 18-22, 25-30) | §10.2 | other slice |
| DER SET order: children in canonical tag order (class order, then number) | §10.3 ¶1, X.680 §8.6 | The caller must say "this is a SET"; SET and SET OF have the same tag 0x31 and cannot be told apart by bytes |
| DER SET OF order: whole child encodings ascending by octet comparison with trailing-zero padding | §11.6 | The caller must say "this is a SET OF". Keep the original byte span of every child (or re-encode canonically) |
| DER: for an untagged CHOICE component the actual outer tag is what is sorted | §10.3 ¶2 | falls out for free |
| An open type's value is a complete TLV | §8.15 | return it as a raw TLV; opening it is the caller's job |
| Encode side: write children in the order given; the SET/SET OF sorters above | §8.10.3, §10.3, §11.6 | |

### 5.2 Needs the schema

| Rule | Clause | Why |
|---|---|---|
| OPTIONAL/DEFAULT: which components are present, positional matching | §8.9.2, §8.9.3, §8.11.2, §8.11.3, X.680 §25.6.1 | needs the component list and its tags |
| DEFAULT omission in DER | §11.5 | needs the default value, and the type's equality (for example default `{}`, default of a SET OF) |
| Which tag an IMPLICIT field really has; whether `[n]` is explicit or implicit; therefore whether a constructed `[n]` holds one child TLV or the contents of a SEQUENCE | §8.14.3, §8.14.4, X.680 §31.2.7 | `A2 03 02 01 05` and a hypothetical implicit-SEQUENCE `[2]` with one INTEGER child are the same bytes |
| Tagged CHOICE is always explicit; IMPLICIT on a CHOICE is invalid | X.680 §31.2.7 c), §31.2.9 | |
| Which alternative a CHOICE holds (match the tag to an alternative); CHOICE has no TLV of its own | §8.13 | |
| Automatic tagging: tags 0,1,2... assigned by position | X.680 §25.10 | compile-time |
| Distinct tags in SET and CHOICE | X.680 §27.3, §29.3 | |
| Whether a `31`-tagged node is SET or SET OF (and so which order rule) | §10.3 versus §11.6 | |
| Form (primitive/constructed) of every tagged or non-SEQUENCE type | §8.14.3, §8.14.4 | |
| CER ordering of an untagged CHOICE inside a SET (smallest tag anywhere inside) | §9.3 ¶2 | needs the alternatives that were *not* chosen |
| What an open type holds (which type, how to decode it) | §8.15, §8.1.2.7 | the choice is outside the bytes (object-set lookup) |
| INSTANCE OF, EMBEDDED PDV, EXTERNAL structure, Table 2, field presence | §8.16, §8.17, §8.18 | fixed SEQUENCE shapes; a typed layer can implement them, the TLV layer treats them as ordinary constructed TLVs (tags 0x28 and 0x2B) |
| Extension markers and constraints do not alter encodings | §8.1.1.4 | |

### 5.3 Consequences for the crate

1. The tree node must keep the exact input bytes of each TLV (offset and length) in addition to class, number,
   form, length form and children, because DER SET OF ordering and "was it minimal" are properties of the
   bytes, and BER re-encoding may differ.
2. Provide `check_set_order(children)` and `check_set_of_order(children)` as separate checks the caller picks.
3. An explicit tag is not special at the TLV level: a constructed context-specific node with one child. Provide
   helpers "expect explicit [n]" and "expect implicit [n] over type T" in the typed layer.
4. Return an open type as an unopened node.
5. The deterministic encoder for SET OF has to encode each element first, then sort the byte strings.
