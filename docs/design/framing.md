# BER/DER framing layer: obligations and test vectors

Sources: Rec. ITU-T X.690 (02/2021) | ISO/IEC 8825-1:2021 and its Erratum 1 (09/2021); Rec. ITU-T X.680 (02/2021) clause 8 and Table 1 (read through `itulib`, cross-checked against `pdftotext -raw` of the PDFs in `the X.690 PDF fetched by primarysource`). 2008 text: X.690 (11/2008) base text, without Corrigenda 1 and 2. No implementation was read. Citations use clause and lettered item; `¶n` is used only where `itu_read` showed the sentence index (verified: 7.3 ¶2, 8.1.3.5 ¶2/¶4/¶5, 8.1.5 ¶1/¶2/¶3, 8.1.2.4.2 ¶1, 10.1 ¶1).

Notation. Bits are numbered 8 (MSB) to 1 (X.690 6.2). Class letters U/A/C/P = universal/application/context-specific/private (8.1.2.2 Table 1). A vector result `OK(class,P|C,number,length)` describes the outermost TLV only; `indef` = indefinite length. "Derived" means the rule follows from two cited sentences but no single sentence says "reject"; the standard contains no receiver-side error rules at all (see section 6).

## Error classes (names used below)

| class | meaning |
|---|---|
| E_TRUNCATED | input ends before the encoding is complete (identifier, length, contents or EOC) |
| E_TAG_LEADING_ZERO | high-tag-number form whose first subsequent octet has bits 7..1 all zero |
| E_TAG_HIGH_FORM_FOR_LOW_NUMBER | high form used for a number 0..30 (derived) |
| E_LEN_RESERVED | initial length octet 0xFF |
| E_INDEFINITE_PRIMITIVE | length 0x80 with P/C bit = 0 |
| E_INDEFINITE_FORBIDDEN | length 0x80 in DER |
| E_LEN_NONMINIMAL | DER only: long form where short form fits, or long form with a leading zero length octet |
| E_EOC_MISPLACED / E_TAG0_NOT_EOC | tag UNIVERSAL 0 where no indefinite-length encoding is open, or not exactly `00 00` |
| E_LEN_OVERRUN | a child's encoding extends past the end of its definite-length parent |
| E_LIMIT_TAG / E_LIMIT_LEN | exceeds the implementation's chosen limit (standard sets none) |
| E_TRAILING_DATA | octets remain after one complete top-level encoding (caller policy; standard silent) |
| E_DER_CONSTRUCTED_STRING | type layer, DER 10.2 |

## 1. Obligations

"Encode" = what a sender does. "Decode BER" / "Decode DER" = what a receiver does: ACCEPT, REJECT, or OPEN (standard silent; the implementation must choose).

| ID | clause and sentence | rule | encode | decode BER | decode DER | error class |
|---|---|---|---|---|---|---|
| F-01 | X.690 §7.3 ¶2 | A receiver claiming BER conformance supports every alternative encoding the sender may choose. | n/a | ACCEPT all alternatives in F-28, F-30, F-37 | n/a (F-02) | n/a |
| F-02 | §7.4 | CER and DER permit no alternative encodings. | n/a | n/a | one encoding per value; reject any other | see F-13, F-14, F-28, F-41 |
| F-03 | §8.1.1.1 ¶1 | An encoding is four components in order: identifier octets, length octets, contents octets, end-of-contents octets. | emit in that order | parse in that order | same | E_TRUNCATED if one is missing |
| F-04 | §8.1.1.2 | EOC octets are absent unless the length octets require them. | no EOC after definite length | `00 00` after a definite-length value is not part of it | same | E_EOC_MISPLACED / E_TRAILING_DATA |
| F-05 | §8.1.2.1 | Identifier octets encode the tag's class and number. | | decode class, number | same | |
| F-06 | §8.1.2.2 ¶1 a) and ¶2 Table 1 | Bits 8-7: universal 00, application 01, context-specific 10, private 11. | | ACCEPT all four | same | |
| F-07 | §8.1.2.2 ¶1 b); §8.1.2.5 | Bit 6 = 0 primitive, 1 constructed. | set from the type (section 3) | read bit 6 | same | |
| F-08 | §8.1.2.2 ¶1 c) and lead-in | Number 0..30: one identifier octet, bits 5-1 = number, bit 5 most significant. | numbers 0..30 always single-octet | ACCEPT | ACCEPT | |
| F-09 | §8.1.2.4 | Number >= 31: a leading octet then one or more subsequent octets. | | ACCEPT | ACCEPT | |
| F-10 | §8.1.2.4.1 a)-c) | Leading octet: class bits, P/C bit, bits 5-1 = 11111. | | ACCEPT | ACCEPT | |
| F-11 | §8.1.2.4.2 a) | Bit 8 of every subsequent octet is 1 except the last, where it is 0. | | continuation loop; input ending while bit 8 = 1 is E_TRUNCATED | same | E_TRUNCATED |
| F-12 | §8.1.2.4.2 b) | Bits 7-1 of the subsequent octets, concatenated big-endian, are the unsigned tag number. | base-128, most significant group first | ACCEPT | ACCEPT | E_LIMIT_TAG if number exceeds the chosen width (F-17) |
| F-13 | §8.1.2.4.2 c) | Bits 7-1 of the first subsequent octet are not all zero (no leading zero group). | never emit `80` first | REJECT | REJECT | E_TAG_LEADING_ZERO |
| F-14 | derived: §8.1.2.2 ¶1 (0..30 "shall comprise a single octet") with §8.1.2.4 (high form for >= 31) | A number 0..30 in the high form (e.g. `9F 1E`) is non-conforming. F-13 does not catch it (first group `1E` is non-zero). | never | REJECT (derived; a lenient decoder is a deliberate deviation) | REJECT | E_TAG_HIGH_FORM_FOR_LOW_NUMBER |
| F-15 | §8.1.2.5 and the per-type clauses | The P/C bit of each universal type is fixed or free per section 3; tagging per §8.14.3, §8.14.4. | see section 3 | at the framing layer P/C is data; the type layer checks it | same | type layer |
| F-16 | §8.1.2.4.3 and Figure 4 as replaced by Erratum 1 | Figure: leading octet (class, P/C, 11111); subsequent octets `1xxxxxxx` ... `1xxxxxxx`, then `0xxxxxxx`; tag number = concatenation of the 7-bit fields. The penultimate octet has bit 8 = 1 (the original 2021 figure drew 0, contradicting F-11). | | follow the text (F-11) | same | |
| F-17 | X.680 (02/2021) §8.2 | Tag number is a non-negative integer in decimal; no maximum stated. X.690 8.1.2.4.2 b) states no maximum count of subsequent octets. | any width | OPEN: choose and document; recommended unsigned 64-bit with overflow check, not 32-bit | same | E_LIMIT_TAG |
| F-18 | X.680 §8.4 Table 1 (verified) | UNIVERSAL 0 reserved for the encoding rules; 15 reserved; 37 and above reserved for addenda; assigned numbers in section 3.1. | never emit universal 0 as a value tag | universal 0 legal only as the EOC closing an open indefinite encoding (F-31); reject elsewhere. Unassigned universal numbers (15, 37+): OPEN; accept at framing, let the type layer refuse | same | E_EOC_MISPLACED |
| F-19 | §8.1.3.1 | Two length forms: definite (§8.1.3.3) and indefinite (§8.1.3.6). | | ACCEPT both | definite only (F-41) | |
| F-20 | §8.1.3.2 a) | A sender uses the definite form if the encoding is primitive. | definite for primitive | REJECT 0x80 on a primitive (derived, F-33) | REJECT | E_INDEFINITE_PRIMITIVE |
| F-21 | §8.1.3.2 b), c) | Constructed: definite or indefinite at sender's option if all available; indefinite if not all available. | either (BER); definite only (DER) | ACCEPT both | indefinite REJECT | E_INDEFINITE_FORBIDDEN |
| F-22 | §8.1.3.3 | Definite form: one or more octets giving the number of contents octets, short or long form at sender's option. | | ACCEPT short and long | see F-41 | |
| F-23 | §8.1.3.4 | Short form: one octet, bit 8 = 0, bits 7-1 = length 0..127 (0 permitted). | use when length <= 127 | ACCEPT | ACCEPT | |
| F-24 | §8.1.3.3 NOTE | The short form is usable only when length <= 127. | | n/a | n/a | |
| F-25 | §8.1.3.5 ¶1, ¶2 a), b) | Long form: initial octet with bit 8 = 1 and bits 7-1 = count of subsequent octets, then those octets. | | ACCEPT | ACCEPT if minimal | |
| F-26 | §8.1.3.5 ¶2 c) | Initial octet 0xFF shall not be used. (Largest usable count is therefore 126, initial octet 0xFE.) | never | REJECT | REJECT | E_LEN_RESERVED |
| F-27 | §8.1.3.5 ¶4 | Subsequent octets, bits 8-1 each, are a big-endian unsigned integer = number of contents octets. | | ACCEPT | ACCEPT | E_LIMIT_LEN above chosen width |
| F-28 | §8.1.3.5 ¶5 (NOTE 2) | A sender may use more length octets than the minimum, including leading zero octets. | minimal recommended | ACCEPT, including leading zero octets and counts above 8; apply the width limit to the value after stripping zeros | REJECT (F-41) | E_LEN_NONMINIMAL (DER) |
| F-29 | §8.1.3.6 ¶1; §8.1.3.6.1 | Indefinite form: a single octet, bit 8 = 1, bits 7-1 = 0 (0x80). `81 00` is a long form of length 0, not indefinite. | | ACCEPT on constructed | REJECT | E_INDEFINITE_FORBIDDEN |
| F-30 | §8.1.3.6.2; §8.1.5 ¶1 | With indefinite length, EOC octets follow the contents; otherwise none. | | contents = complete encodings until the first EOC at this nesting level; missing EOC is E_TRUNCATED | n/a | E_TRUNCATED |
| F-31 | §8.1.5 ¶2 | EOC is exactly two zero octets. | `00 00` | tag 0 with length != 0 is not EOC | n/a | E_TAG0_NOT_EOC |
| F-32 | §8.1.5 ¶3 NOTE | EOC can be viewed as UNIVERSAL 0, primitive, no contents. | | informative | | |
| F-33 | derived: §8.1.3.2 a) with §8.1.3.6 and §8.1.5 | Indefinite length on a primitive encoding is non-conforming (primitive contents hold no nested encodings to end with EOC). Decidable from the identifier octet alone. | never | REJECT | REJECT | E_INDEFINITE_PRIMITIVE |
| F-34 | §8.1.4 | Contents: zero, one or more octets, defined per type. | | | | |
| F-35 | §8.6.3, §8.7.3, §8.9.2, §8.10.2, §8.11.2, §8.14.3 | Contents of a constructed encoding are complete nested encodings. With a definite parent every child must end within it; with an indefinite parent children are read until EOC. | | REJECT overrun | REJECT overrun | E_LEN_OVERRUN |
| F-36 | §8.14.3, §8.14.4 a) | Explicit tag: always constructed, contents = complete base encoding. Implicit tag: constructed iff the base encoding is. | | type layer | type layer | |
| F-37 | §8.6.1, §8.7.1, §8.23.3, §8.23.6 | BIT STRING, OCTET STRING and restricted character strings may be primitive or constructed at sender's option; receivers handle all forms. Segment tags are universal 3 / 4 (8.6.4.1 NOTE 2, 8.7.3.2 NOTE 2) and nest recursively. | | ACCEPT both | primitive only (F-42) | E_DER_CONSTRUCTED_STRING |
| F-38 | §8.13, §8.15 | CHOICE and open types add no framing: the encoding is the chosen type's. | | | | |
| F-41 | §10.1 ¶1 | DER: definite form only, in the minimum number of octets. Minimum = short form for 0..127; otherwise long form with the smallest count and no leading zero length octet. | emit minimal | n/a | REJECT indefinite; REJECT non-minimal long form (`81 7F`, `82 00 80`, `81 00`) | E_INDEFINITE_FORBIDDEN, E_LEN_NONMINIMAL |
| F-42 | §10.2 | DER: constructed form not used for BIT STRING, OCTET STRING, restricted character strings. | | n/a | REJECT constructed (type layer) | E_DER_CONSTRUCTED_STRING |
| F-43 | §9.1 (CER, for completeness) | CER: constructed uses indefinite; primitive uses fewest length octets. | | | | out of scope |
| F-44 | §10 lead-in, §8.1.2.4.2 | DER adds no tag-octet rule beyond BER's; tag minimality is already mandatory in BER (F-13, F-14). | | | same checks as BER | |

(IDs F-39 and F-40 are unused; the primitive/constructed content is in section 3.)

## 2. What a decoder SHALL accept, SHALL reject, and what is open

BER decode.
- SHALL accept (7.3 ¶2 with 8.1.3.2 b) and 8.1.3.5 NOTE 2): short and long definite lengths including non-minimal ones and leading zero length octets; indefinite length on constructed encodings, closed by EOC; constructed BIT STRING, OCTET STRING and restricted-string forms, nested; all four classes; tag numbers >= 31 in the high form.
- SHALL reject (each is a "shall not" on the sender in the cited clause; rejection by the receiver is derived, see section 6): length octet 0xFF (8.1.3.5 ¶2 c); a high-form tag whose first subsequent octet is `80` (8.1.2.4.2 c); a high-form tag for a number below 31 (F-14); indefinite length with bit 6 = 0 (F-33); an EOC that is not exactly `00 00`; UNIVERSAL 0 anywhere except as the EOC closing an open indefinite encoding; a child that overruns a definite parent; truncated input.
- OPEN: largest tag number; largest length value; largest count of length octets (the cap of 126 is implied by 0xFF being excluded, so a length up to 2^1008-1 is expressible); nesting depth; trailing octets after one top-level encoding; unassigned universal tag numbers. The implementation must choose and document each. A safe minimum: tag number as u64 with checked shifts (vectors `tag 2^32`, `tag 2^64-1`, `tag 2^64`); length taken as the value after stripping leading zeros and compared with the remaining input before any allocation; a cap on constructed nesting depth (the standard sets none; unbounded recursion is a resource hazard, not a conformance duty).

DER decode.
- Everything BER rejects, plus: indefinite length (10.1); a long-form length that is not minimal (10.1); constructed BIT STRING / OCTET STRING / restricted strings (10.2, type layer).
- SHALL accept: short form 0..127; long form in minimal octets for 128 and above; all four classes; high-form tags with a non-zero first group.
- OPEN: the same limits as BER.
- SET ordering (10.3) and the clause 11 value rules (BOOLEAN 0xFF, unused bits zero, defaults omitted, SET OF order, time formats) are type-layer rules and not covered here.

## 3. Primitive versus constructed (what the type layer feeds the framing layer)

Always primitive: BOOLEAN (8.2.1), INTEGER (8.3.1), ENUMERATED (8.4 NOTE), REAL (8.5.1), NULL (8.8.1), OBJECT IDENTIFIER (8.19.1), RELATIVE-OID (8.20.1), OID-IRI (8.21.1), RELATIVE-OID-IRI (8.22.1), TIME and DATE, TIME-OF-DAY, DATE-TIME, DURATION (8.26.1.1 to 8.26.5.1).
Always constructed: SEQUENCE (8.9.1), SEQUENCE OF (8.10.1), SET (8.11.1), SET OF (8.12.1); EXTERNAL, embedded-pdv, unrestricted character string and instance-of, each being "the BER encoding of" a sequence type (8.16.1, 8.17.1, 8.18.1, 8.24.1).
Sender's option: BIT STRING (8.6.1), OCTET STRING (8.7.1), every restricted character string (8.23.3: encoded as `[UNIVERSAL x] IMPLICIT OCTET STRING`, with 8.23.6), and GeneralizedTime, UTCTime, ObjectDescriptor (8.25 encodes them via their X.680 definitions, implicit VisibleString / VisibleString / GraphicString: X.680 §46.3, §47.3, §48.3), hence also either form. BER receivers accept both; DER primitive for the string types named in 10.2.
Tagged types: explicit = constructed (8.14.3); implicit = form of the base (8.14.4 a). CHOICE: form of the chosen type (8.13).

### 3.1 Universal tag numbers (X.680 (02/2021) §8.4 Table 1)

| number | assignment |
|---|---|
| 0 | reserved for the encoding rules (EOC) |
| 1 BOOLEAN, 2 INTEGER, 3 BIT STRING, 4 OCTET STRING, 5 NULL, 6 OBJECT IDENTIFIER | |
| 7 | object descriptor |
| 8 | EXTERNAL and INSTANCE OF |
| 9 REAL, 10 ENUMERATED, 11 EMBEDDED PDV | |
| 12 | UTF8String |
| 13 | RELATIVE-OID |
| 14 | the TIME type |
| 15 | reserved for future editions |
| 16 | SEQUENCE and SEQUENCE OF |
| 17 | SET and SET OF |
| 18-22, 25-30 | character string types (Table 8, §41.1: 18 Numeric, 19 Printable, 20 Teletex, 21 Videotex, 22 IA5, 25 Graphic, 26 Visible, 27 General, 28 Universal, 30 BMP; 29 CHARACTER STRING per X.680 §44.2; 12 UTF8String) |
| 23, 24 | UTCTime, GeneralizedTime |
| 31, 32, 33, 34 | DATE, TIME-OF-DAY, DATE-TIME, DURATION |
| 35, 36 | OID-IRI, RELATIVE-OID-IRI |
| 37 and above | reserved for addenda |

Tag classes (X.680 §8.1): universal, application, private, context-specific; X.680 §31.2.5: a user may not write UNIVERSAL in a tag except for types defined in the Recommendation itself.

## 4. Test vectors

Notation `[00 xN]` = N octets of 0x00. Reference results were checked by a small framing parser written only from section 1 (script `gen_vectors.py` in this directory: 104 vectors, 0 mismatches; the check accepts the alternatives marked in brackets). A BER/DER pair with the same expectation is one row; `vectors.tsv` has the same data tab-separated.

Where a row mixes sources (for example a length octet from a standard example plus an identifier added by hand) it says so. Rows named "trunc ... prefix k/n" are every proper prefix of: A = `30 06 02 01 05 04 01 41` (k 0..7), B = `9F 81 00 82 01 00` + 256 x `00` (k 0..8: every header position and the first 3 contents octets), C = `30 80 02 01 05 00 00` (k 0..6; DER gives E_INDEFINITE_FORBIDDEN once the length octet `80` is present, k >= 2).

Annex A record, full hex (from X.690 Annex A.3, rebuilt from the figure's columns; every length in the figure is reproduced by the builder):
`60 81 85 61 10 1A 04 4A 6F 68 6E 1A 01 50 1A 05 53 6D 69 74 68 A0 0A 1A 08 44 69 72 65 63 74 6F 72 42 01 33 A1 0A 43 08 31 39 37 31 30 39 31 37 A2 12 61 10 1A 04 4D 61 72 79 1A 01 54 1A 05 53 6D 69 74 68 A3 42 31 1F 61 11 1A 05 52 61 6C 70 68 1A 01 54 1A 05 53 6D 69 74 68 A0 0A 43 08 31 39 35 37 31 31 31 31 31 1F 61 11 1A 05 53 75 73 61 6E 1A 01 42 1A 05 4A 6F 6E 65 73 A0 0A 43 08 31 39 35 39 30 37 31 37`

No DER example encoding was found in the clauses and annexes read (8.x examples, Annex A); every DER row is derived from 10.1, 10.2 and the BER clauses.


| hex input | mode | expected | provenance |
|---|---|---|---|
| 02 01 05 | BER, DER | OK(U,P,2,1) | id: constructed by hand from X.690 8.1.2.2: 0x02 = class 00, P/C 0, number 00010 |
| 30 00 | BER, DER | OK(U,C,16,0) | id: constructed by hand from 8.1.2.2, 8.1.2.5: 0x30 = 00 1 10000 (SEQUENCE, X.680 Table 1) |
| 40 00 | BER, DER | OK(A,P,0,0) | id: constructed by hand from 8.1.2.2 Table 1: 0x40 = 01 0 00000 |
| 80 00 | BER, DER | OK(C,P,0,0) | id: constructed by hand from 8.1.2.2 Table 1: 0x80 = 10 0 00000 |
| C0 00 | BER, DER | OK(P,P,0,0) | id: constructed by hand from 8.1.2.2 Table 1: 0xC0 = 11 0 00000 |
| A0 00 | BER, DER | OK(C,C,0,0) | id: constructed by hand from 8.1.2.2, 8.1.2.5: 0xA0 = 10 1 00000 |
| 9E 00 | BER, DER | OK(C,P,30,0) | tag 30: constructed by hand from 8.1.2.2: 0x9E = 10 0 11110; 30 is the largest low-form number |
| 9F 1F 00 | BER, DER | OK(C,P,31,0) | tag 31: constructed by hand from 8.1.2.4.1 (leading 0x9F = 10 0 11111) + 8.1.2.4.2 (31 = 0x1F in one 7-bit group) |
| 9F 7F 00 | BER, DER | OK(C,P,127,0) | tag 127: constructed by hand from 8.1.2.4.2: 127 = 1111111b, one group, bit 8 clear |
| 9F 81 00 00 | BER, DER | OK(C,P,128,0) | tag 128: constructed by hand from 8.1.2.4.2: 128 = 0000001 0000000b -> 0x81 0x00 |
| 9F FF 7F 00 | BER, DER | OK(C,P,16383,0) | tag 16383: constructed by hand from 8.1.2.4.2: 16383 = 1111111 1111111b -> 0xFF 0x7F |
| 9F 81 80 00 00 | BER, DER | OK(C,P,16384,0) | tag 16384: constructed by hand from 8.1.2.4.2: 16384 = 0000001 0000000 0000000b -> 0x81 0x80 0x00 |
| BF 1F 00 | BER, DER | OK(C,C,31,0) | tag 31 constructed: constructed by hand from 8.1.2.4.1, 8.1.2.5: 0xBF = 10 1 11111 |
| 5F 1F 00 | BER, DER | OK(A,P,31,0) | tag 31 application: constructed by hand from 8.1.2.4.1: 0x5F = 01 0 11111 |
| FF 1F 00 | BER, DER | OK(P,C,31,0) | tag 31 private constructed: constructed by hand from 8.1.2.4.1: 0xFF = 11 1 11111 |
| 9F 90 80 80 80 00 00 | BER, DER | OK(C,P,4294967296,0)  [E_LIMIT_TAG if tag stored in u32] | tag 2^32: constructed by hand from 8.1.2.4.2: 2^32 = 0010000 0000000 0000000 0000000 0000000b; the standard sets no upper bound |
| 9F 81 FF FF FF FF FF FF FF FF 7F 00 | BER, DER | OK(C,P,18446744073709551615,0) | tag 2^64-1: constructed by hand from 8.1.2.4.2: 2^64-1 = 1 then nine groups of 1111111b (64 ones = 1 + 9*7) |
| 9F 82 80 80 80 80 80 80 80 80 00 00 | BER, DER | E_LIMIT_TAG  [policy: OK if tag is unbounded] | tag 2^64: constructed by hand from 8.1.2.4.2: 2^64 = 0000010 then eight 0000000 groups, last octet bit 8 clear (10 octets after 0x9F) |
| 9F 1E 00 | BER, DER | E_TAG_HIGH_FORM_FOR_LOW_NUMBER | tag 30 in high form: constructed by hand from 8.1.2.2 (0..30 'shall comprise a single octet') and 8.1.2.4 (high form only for >= 31); derived, no sentence says 'reject' |
| 1F 00 00 | BER, DER | E_TAG_LEADING_ZERO | tag 0 in high form: constructed by hand from 8.1.2.4.2 c): first subsequent octet 0x00 has bits 7..1 all zero |
| 9F 80 01 00 | BER, DER | E_TAG_LEADING_ZERO | tag leading zero group: constructed by hand from 8.1.2.4.2 c): first subsequent octet 0x80 has bits 7..1 all zero (value would be 1) |
| 9F 80 80 7F 00 | BER, DER | E_TAG_LEADING_ZERO | tag leading zero group x2: constructed by hand from 8.1.2.4.2 c) |
| 9F 81 | BER, DER | E_TRUNCATED | tag cont. at end: constructed by hand from 8.1.2.4.2 a): bit 8 set means another octet must follow; input ends |
| 1F 1F 00 | BER, DER | OK(U,P,31,0) | universal tag 31 high form: constructed by hand from 8.1.2.4 with X.680 Table 1 (UNIVERSAL 31 = DATE); framing only |
| 00 01 05 | BER, DER | E_TAG0_NOT_EOC | reserved tag 0 (not EOC): constructed by hand from X.680 8.4 Table 1 (UNIVERSAL 0 reserved for encoding rules) and X.690 8.1.5 (EOC = two zero octets) |
| 04 00 | BER, DER | OK(U,P,4,0) | len 0: constructed by hand from 8.1.3.4: short form, 0 permitted |
| 04 26 [41 x38] | BER, DER | OK(U,P,4,38) | len 38: identifier by hand (OCTET STRING, 8.1.2.2); length octet 0x26 from X.690 8.1.3.4 EXAMPLE (L=38 = 00100110b), contents 38 x 0x41 |
| 04 7F [00 x127] | BER, DER | OK(U,P,4,127) | len 127: constructed by hand from 8.1.3.4 / 8.1.3.3 NOTE: 127 is the largest short form |
| 04 81 80 [00 x128] | BER, DER | OK(U,P,4,128) | len 128: constructed by hand from 8.1.3.5: 128 > 127, so long form, 1 subsequent octet 0x80 |
| 04 81 C9 [00 x201] | BER, DER | OK(U,P,4,201) | len 201: identifier by hand; length octets 81 C9 from X.690 8.1.3.5 EXAMPLE (L=201) |
| 04 81 FF [00 x255] | BER, DER | OK(U,P,4,255) | len 255: constructed by hand from 8.1.3.5: 255 fits one subsequent octet |
| 04 82 01 00 [00 x256] | BER, DER | OK(U,P,4,256) | len 256: constructed by hand from 8.1.3.5: 256 needs 2 subsequent octets 0x01 0x00 |
| 04 82 FF FF [00 x65535] | BER, DER | OK(U,P,4,65535) | len 65535: constructed by hand from 8.1.3.5 |
| 04 83 01 00 00 [00 x65536] | BER, DER | OK(U,P,4,65536) | len 65536: constructed by hand from 8.1.3.5: 3 subsequent octets |
| 04 81 01 41 | BER | OK(U,P,4,1) | non-minimal 81 01: constructed by hand from 8.1.3.5 NOTE 2 (BER allows extra length octets); 10.1 (DER minimum octets): 1 < 128 must be short form |
| 04 81 01 41 | DER | E_LEN_NONMINIMAL | (same input) |
| 04 81 00 | BER | OK(U,P,4,0) | non-minimal 81 00: constructed by hand from 8.1.3.5 NOTE 2; 10.1. Note 0x81 0x00 is not the indefinite form (that is the single octet 0x80) |
| 04 81 00 | DER | E_LEN_NONMINIMAL | (same input) |
| 04 81 7F [00 x127] | BER | OK(U,P,4,127) | non-minimal 81 7F: constructed by hand from 8.1.3.3 NOTE (127 fits short form); 10.1 |
| 04 81 7F [00 x127] | DER | E_LEN_NONMINIMAL | (same input) |
| 04 82 00 80 [00 x128] | BER | OK(U,P,4,128) | non-minimal 82 00 80: constructed by hand from 8.1.3.5 NOTE 2; 10.1: leading zero length octet |
| 04 82 00 80 [00 x128] | DER | E_LEN_NONMINIMAL | (same input) |
| 04 82 00 01 41 | BER | OK(U,P,4,1) | non-minimal 82 00 01: constructed by hand from 8.1.3.5 NOTE 2; 10.1 |
| 04 82 00 01 41 | DER | E_LEN_NONMINIMAL | (same input) |
| 04 84 00 00 00 01 41 | BER | OK(U,P,4,1) | non-minimal 84: constructed by hand from 8.1.3.5 NOTE 2; 10.1 |
| 04 84 00 00 00 01 41 | DER | E_LEN_NONMINIMAL | (same input) |
| 04 89 00 00 00 00 00 00 00 00 01 41 | BER | OK(U,P,4,1)  [E_LIMIT_LEN only if the decoder fails to strip leading zeros] | 9 length octets, value 1: constructed by hand from 8.1.3.5 ¶4 (value of the octets) + NOTE 2: leading zero octets are legal in BER, so the 64-bit limit applies to the value, not the octet count |
| 04 89 00 00 00 00 00 00 00 00 01 41 | DER | E_LEN_NONMINIMAL | (same input) |
| 04 FF 01 41 | BER, DER | E_LEN_RESERVED | reserved length 0xFF: constructed by hand from 8.1.3.5 c): initial octet 11111111b shall not be used |
| 04 FE [00 x125] 01 41 | BER | OK(U,P,4,1)  [leading zeros; E_LIMIT_LEN if decoder bounds the octet count] | length-of-length 126 (0xFE): constructed by hand from 8.1.3.5 b): 0xFE = 126 subsequent octets is the largest count 8.1.3.5 c) leaves |
| 04 FE [00 x125] 01 41 | DER | E_LEN_NONMINIMAL | (same input) |
| 04 89 01 00 00 00 00 00 00 00 00 | BER, DER | E_LIMIT_LEN  [length 2^64; cannot be satisfied by any buffer] | length > 2^64-1: constructed by hand from 8.1.3.5 ¶4: 9 subsequent octets, value 2^64; standard sets no upper bound |
| 04 88 FF FF FF FF FF FF FF FF | BER, DER | E_TRUNCATED | length 2^64-1, no contents: constructed by hand from 8.1.3.5 ¶4: value 2^64-1 octets of contents announced, none present |
| 04 81 | BER, DER | E_TRUNCATED | length truncated: constructed by hand from 8.1.3.5: 1 subsequent octet announced, none present |
| 04 82 01 | BER, DER | E_TRUNCATED | length truncated 2: constructed by hand from 8.1.3.5: 2 announced, 1 present |
| 04 05 41 42 | BER, DER | E_TRUNCATED | contents truncated: constructed by hand from 8.1.3.4: 5 contents octets announced, 2 present |
| 04 | BER, DER | E_TRUNCATED | no length octet: constructed by hand from 8.1.1.1: length octets are a required component |
| 04 80 41 00 00 | BER, DER | E_INDEFINITE_PRIMITIVE | indef on primitive: constructed by hand from 8.1.3.2 a) (primitive: definite form) with 8.1.3.6 (indefinite form delimits nested encodings, impossible in primitive contents) |
| 02 80 05 00 00 | BER, DER | E_INDEFINITE_PRIMITIVE | indef on primitive INTEGER: constructed by hand from 8.1.3.2 a) |
| 30 80 00 00 | BER | OK(U,C,16,indef) | indef empty SEQUENCE: constructed by hand from 8.1.3.6, 8.1.5 (BER); 10.1 (DER: definite form only) |
| 30 80 00 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 02 01 05 00 00 | BER | OK(U,C,16,indef) | indef SEQUENCE{INTEGER}: constructed by hand from 8.1.3.6, 8.1.5; 10.1 |
| 30 80 02 01 05 00 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 30 80 00 00 00 00 | BER | OK(U,C,16,indef) | indef nested: constructed by hand from 8.1.5: each indefinite encoding has its own EOC; the first EOC closes the inner |
| 30 80 30 80 00 00 00 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 30 80 00 00 | BER | E_TRUNCATED | indef nested, outer EOC missing: constructed by hand from 8.1.3.6.2: EOC shall be present for the outer encoding |
| 30 80 30 80 00 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 02 01 05 | BER | E_TRUNCATED | indef: EOC missing: constructed by hand from 8.1.3.6.2 |
| 30 80 02 01 05 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 02 01 05 00 | BER | E_TRUNCATED | indef: half EOC: constructed by hand from 8.1.5: EOC is two zero octets; one present |
| 30 80 02 01 05 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 00 01 05 | BER | E_TAG0_NOT_EOC | indef: tag 0 with length 1 instead of EOC: constructed by hand from 8.1.5 ¶2 (EOC = 00 00 exactly); tag 0 is reserved (X.680 Table 1) |
| 30 80 00 01 05 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 04 30 80 00 00 | BER | OK(U,C,16,4) | definite parent holding indef child: constructed by hand from 8.1.3.3 / 8.1.3.6: the definite parent's length counts the child's EOC octets |
| 30 04 30 80 00 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 03 30 80 00 | BER | E_TRUNCATED  [E_LEN_OVERRUN also reasonable] | definite parent, indef child EOC cut: constructed by hand from 8.1.3.6.2 + 8.1.1.1: parent contents must hold the child's complete encoding |
| 30 03 30 80 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 00 00 | BER, DER | E_EOC_MISPLACED | EOC at top level: constructed by hand from 8.1.5 ¶1: EOC present only where length was indefinite (8.1.1.2) |
| 30 02 00 00 | BER, DER | E_EOC_MISPLACED | EOC inside definite parent: constructed by hand from 8.1.1.2 / 8.1.5 ¶1: parent is definite, so no EOC is required or permitted; tag 0 reserved (X.680 Table 1) |
| 30 00 00 00 | BER, DER | OK(U,C,16,0)+remainder(2)  [whole-buffer decode: E_TRAILING_DATA] | EOC after complete value: constructed by hand from 8.1.1.2: the 00 00 after a definite-length value belongs to no encoding |
| 20 00 | BER, DER | E_TAG0_NOT_EOC | constructed tag 0 (0x20 0x00): constructed by hand from X.680 Table 1 (UNIVERSAL 0 reserved); 8.1.5 NOTE gives EOC as primitive only |
| 30 03 02 02 05 | BER, DER | E_LEN_OVERRUN | child overruns parent: constructed by hand from 8.9.2 / 8.1.4: contents are complete encodings; child needs 4 octets, parent holds 3 |
| 30 02 02 01 05 | BER, DER | E_LEN_OVERRUN | child overruns (trailing byte outside): constructed by hand from same: parent content is '02 01' only; the 05 is outside the parent |
| 30 05 02 01 05 04 01 41 | BER, DER | E_LEN_OVERRUN | parent too short for 2nd child: constructed by hand from 8.1.4; parent contents end after '04 01' |
| 01 01 FF | BER, DER | OK(U,P,1,1) | BOOLEAN TRUE: from X.690 8.2.2 EXAMPLE |
| 05 00 | BER, DER | OK(U,P,5,0) | NULL: from X.690 8.8.2 EXAMPLE |
| 30 0A 16 05 53 6D 69 74 68 01 01 FF | BER, DER | OK(U,C,16,10) | 8.9 SEQUENCE example: from X.690 8.9.3 EXAMPLE (30 0A / 16 05 'Smith' / 01 01 FF) |
| 43 05 4A 6F 6E 65 73 | BER, DER | OK(A,P,3,5) | 8.14 Type2: from X.690 8.14.4 EXAMPLE (Type2) |
| A2 07 43 05 4A 6F 6E 65 73 | BER, DER | OK(C,C,2,7) | 8.14 Type3: from X.690 8.14.4 EXAMPLE (Type3, explicit [2]) |
| 67 07 43 05 4A 6F 6E 65 73 | BER, DER | OK(A,C,7,7) | 8.14 Type4: from X.690 8.14.4 EXAMPLE (Type4) |
| 82 05 4A 6F 6E 65 73 | BER, DER | OK(C,P,2,5) | 8.14 Type5: from X.690 8.14.4 EXAMPLE (Type5, implicit [2] over implicit [APPLICATION 3]) |
| 03 07 04 0A 3B 5F 29 1C D0 | BER, DER | OK(U,P,3,7) | 8.6.4.2 BIT STRING primitive: from X.690 8.6.4.2 EXAMPLE (first form) |
| 23 80 03 03 00 0A 3B 03 05 04 5F 29 1C D0 00 00 | BER | OK(U,C,3,indef) | 8.6.4.2 BIT STRING constructed: from X.690 8.6.4.2 EXAMPLE (second form); DER verdict from 10.1 and 10.2 |
| 23 80 03 03 00 0A 3B 03 05 04 5F 29 1C D0 00 00 | DER | E_INDEFINITE_FORBIDDEN  (also 10.2: constructed BIT STRING forbidden) | (same input) |
| 1A 05 4A 6F 6E 65 73 | BER, DER | OK(U,P,26,5) | 8.23.5 primitive VisibleString: from X.690 8.23.6 example, primitive form |
| 3A 09 04 03 4A 6F 6E 04 02 65 73 | BER | OK(U,C,26,9) | 8.23.5 constructed definite: from X.690 8.23.6 example, constructor form, definite length |
| 3A 09 04 03 4A 6F 6E 04 02 65 73 | DER | OK(U,C,26,9) at framing; type layer E_DER_CONSTRUCTED_STRING (10.2) | (same input) |
| 3A 80 04 03 4A 6F 6E 04 02 65 73 00 00 | BER | OK(U,C,26,indef) | 8.23.5 constructed indefinite: from X.690 8.23.6 example, constructor form, indefinite length |
| 3A 80 04 03 4A 6F 6E 04 02 65 73 00 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 60 81 85 61 10 1A 04 4A 6F 68 6E 1A 01 50 1A 05 53 6D 69 74 68 A0 0A 1A 08 44 69 72 65 63 74 6F 72 42 01 33 A1 0A 43 ... | BER | OK(A,C,0,133) | Annex A PersonnelRecord: from X.690 Annex A.3 (every identifier, length and string re-keyed from the figure; the standard's lengths 60 81 85, 61 10, A0 0A, 42 01 33, A1 0A, A2 12, A3 42, 31 1F, 61 11 all reproduced by the builder) |
| 60 81 85 61 10 1A 04 4A 6F 68 6E 1A 01 50 1A 05 53 6D 69 74 68 A0 0A 1A 08 44 69 72 65 63 74 6F 72 42 01 33 A1 0A 43 ... | DER | OK(A,C,0,133) at framing; SET order in the record is not DER order (10.3), a type-layer matter | (same input) |
| (empty) | BER, DER | E_TRUNCATED | trunc A prefix 0/8: constructed by hand from 8.1.1.1 (all components required): every proper prefix of a complete encoding |
| 30 | BER, DER | E_TRUNCATED | trunc A prefix 1/8: constructed by hand from 8.1.1.1 (all components required): every proper prefix of a complete encoding |
| 30 06 | BER, DER | E_TRUNCATED | trunc A prefix 2/8: constructed by hand from 8.1.1.1 (all components required): every proper prefix of a complete encoding |
| 30 06 02 | BER, DER | E_TRUNCATED | trunc A prefix 3/8: constructed by hand from 8.1.1.1 (all components required): every proper prefix of a complete encoding |
| 30 06 02 01 | BER, DER | E_TRUNCATED | trunc A prefix 4/8: constructed by hand from 8.1.1.1 (all components required): every proper prefix of a complete encoding |
| 30 06 02 01 05 | BER, DER | E_TRUNCATED | trunc A prefix 5/8: constructed by hand from 8.1.1.1 (all components required): every proper prefix of a complete encoding |
| 30 06 02 01 05 04 | BER, DER | E_TRUNCATED | trunc A prefix 6/8: constructed by hand from 8.1.1.1 (all components required): every proper prefix of a complete encoding |
| 30 06 02 01 05 04 01 | BER, DER | E_TRUNCATED | trunc A prefix 7/8: constructed by hand from 8.1.1.1 (all components required): every proper prefix of a complete encoding |
| (empty) | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 0/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| 9F | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 1/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| 9F 81 | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 2/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| 9F 81 00 | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 3/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| 9F 81 00 82 | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 4/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| 9F 81 00 82 01 | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 5/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| 9F 81 00 82 01 00 | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 6/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| 9F 81 00 82 01 00 00 | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 7/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| 9F 81 00 82 01 00 00 00 | BER, DER | E_TRUNCATED | trunc B (high tag, long len) prefix 8/262: constructed by hand from 8.1.2.4.2 / 8.1.3.5: header prefixes (and first 3 contents octets) |
| (empty) | BER, DER | E_TRUNCATED | trunc C (indefinite) prefix 0/7: constructed by hand from 8.1.3.6.2 / 8.1.5 |
| 30 | BER, DER | E_TRUNCATED | trunc C (indefinite) prefix 1/7: constructed by hand from 8.1.3.6.2 / 8.1.5 |
| 30 80 | BER | E_TRUNCATED | trunc C (indefinite) prefix 2/7: constructed by hand from 8.1.3.6.2 / 8.1.5 |
| 30 80 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 02 | BER | E_TRUNCATED | trunc C (indefinite) prefix 3/7: constructed by hand from 8.1.3.6.2 / 8.1.5 |
| 30 80 02 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 02 01 | BER | E_TRUNCATED | trunc C (indefinite) prefix 4/7: constructed by hand from 8.1.3.6.2 / 8.1.5 |
| 30 80 02 01 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 02 01 05 | BER | E_TRUNCATED | trunc C (indefinite) prefix 5/7: constructed by hand from 8.1.3.6.2 / 8.1.5 |
| 30 80 02 01 05 | DER | E_INDEFINITE_FORBIDDEN | (same input) |
| 30 80 02 01 05 00 | BER | E_TRUNCATED | trunc C (indefinite) prefix 6/7: constructed by hand from 8.1.3.6.2 / 8.1.5 |
| 30 80 02 01 05 00 | DER | E_INDEFINITE_FORBIDDEN | (same input) |

## 5. Edition differences, X.690 (11/2008) to (02/2021)

Method: `itu_compare` on 8.1.2.4.2, 8.1.3.2, 8.1.3.5, 8.1.5, 9.1, 10.1, 8.1.1.2; then a sentence-set diff of the raw `pdftotext -raw` text of both editions over 7 to 8.2, 8.1.3 to 8.2, 8.6 to 8.8, 8.9 to 8.15 and 9 to 12, with page headers stripped.

Differences that change an encoding or a validity rule in the framing layer: none found. Clauses 7.3, 7.4, 8.1.1 to 8.1.5, 9.1, 10.1, 10.2 and 11.1 to 11.2 are the same sentence for sentence. The 2008 text has the same 8.1.2.4.2 c) (no leading zero group) and the same 0xFF reservation.

Differences found, all non-normative:
1. Figure 4 (high-tag-number identifier octets): the 02/2021 print draws the penultimate subsequent octet with bit 8 = 0, which contradicts 8.1.2.4.2 a); Erratum 1 (09/2021) replaces it with bit 8 = 1 (verified, section 7). The 2008 figure's extracted text reads "1 1 1 1 1 1 1 1 0", which fits bit 8 = 1 for the penultimate octet, but the 2008 figure is a drawing and I could not check it visually (not proof).
2. Editorial: "ITU-T Rec. X.680" became "Rec. ITU-T X.680"; "a octetstring" became "an octetstring" (8.7.3.2); the heading "End-of-contents octets" in 2008 is body text in 2021 (8.1.5); the Figure 1 and 2 captions and the 2008 figure labels (X.690_F1..F4) changed; the CER set-order example is relabelled "Example"/"EXAMPLE".
3. Outside my slice: both editions already contain TIME, DATE, TIME-OF-DAY, DATE-TIME, DURATION (8.26) and OID-IRI (8.21, 8.22); I did not diff clauses 8.2 to 8.5, 8.19 to 8.26 or 11.3 to 11.9 sentence by sentence beyond the 9 to 12 diff above (which showed only reflow and case differences).

Not checked: X.680 (11/2008) Table 1 (the library has no X.680 2008 corpus, `itu_compare` refused it); X.690 Corrigendum 1 (10/2011) and Corrigendum 2 (03/2014), which are not applied to the 2008 base text; the 08/2015 edition (present in the corpus, not compared).

## 6. Statements not found in X.690

Not folded into the table above as obligations; each was searched for (grep of the 02/2021 text for reject, error, invalid, malformed, maximum, limit, largest, depth, nest, trailing; `itu_search`) and is absent.
1. Any clause saying what a receiver does on malformed input. The standard states only what a sender "shall" do and what conforms; "reject" is derived from non-conformance. This covers F-13, F-14, F-26, F-33 and F-35.
2. A maximum tag number, or a maximum count of identifier octets.
3. A maximum length value; the only bound is the 0xFF reservation (126 length octets).
4. A maximum nesting depth of constructed encodings.
5. An explicit sentence forbidding indefinite length on a primitive encoding (derived from 8.1.3.2 a) and 8.1.3.6).
6. An explicit sentence forbidding a number 0..30 in the high form (derived from 8.1.2.2 and 8.1.2.4).
7. An explicit sentence forbidding UNIVERSAL 0 as a value tag inside a definite-length encoding (the prohibition is in X.680 Table 1, "reserved for use by the encoding rules", and in 8.1.5's description of EOC).
8. How to treat octets after the end of one top-level encoding.
9. How to treat unassigned universal tag numbers (15, 37 and above) when decoding.
10. DER minimum-length wording for the tag octets: 10.1 speaks only of length octets; tag minimality comes from 8.1.2.4.2 c) and 8.1.2.2.
11. DER-specific test encodings (none found in the examples and Annexes A and C headings read).
12. Any statement that DER decoders must accept BER inputs; 7.4 and 10.1 say the opposite direction (only the single DER form is permitted).

## 7. What was verified against the PDF, and what was not

Verified with `pdftotext -raw` (text of 8.1.1 to 8.1.5, 8.2 to 8.14 examples, 8.23, 9 to 11, Annex A, and X.680 §8 including Table 1 and Table 8) and read against the `itulib` extraction: all clause wording and the ¶ indices cited. Table 1 of X.680 was read from the raw text; per-type tag assignments cross-checked in X.680 for 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 13, 16, 17, 29, 35, 36 and the restricted strings (Table 8); I did not cross-check 14 and 31 to 34 beyond the table row.
Table 1 of X.690 (class bits) was read from raw text in 8.1.2.2.
Figure 4 and the Erratum 1 replacement are drawings with no text layer. I extracted the embedded images from both PDFs (`orig_p12_Image199.png` from X.690-2021-02.pdf, `Image31.png` from the erratum) and looked at them. Original: leading octet (Class, P/C, 1 1 1 1 1); 2nd octet with bit 8 = 1; a following octet with bit 8 = 1; the penultimate octet with bit 8 = 0 (typo); last octet with bit 8 = 0. Erratum: the same with the penultimate octet's bit 8 = 1. Label on the erratum image: `X.690(21)-Err1(21)_F04`. Figure 3 is unchanged.
Not verified: the figures 1 and 2 (structure diagrams); Annex A.3 was read as columnar text and the hex rebuilt (lengths checked arithmetically, not against a rendered page); the reference parser that checks the vectors is written from this document, so it checks internal consistency, not the standard.
