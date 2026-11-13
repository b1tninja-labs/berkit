# ASN.1 BER/DER: restricted character strings and time types - obligation table

Sources read (primary): ITU-T X.690 (02/2021) | ISO/IEC 8825-1:2021 (itulib clauses via `itu_read`, and
`pdftotext -raw` of `X.690-2021-02.pdf`), Erratum 1 (09/2021) (covers Figure 4 only; no effect on this slice),
ITU-T X.680 (02/2021) via `pdftotext -raw` and `-layout` of `X.680-2021-02.pdf`, X.690 (11/2008) and (08/2015)
for edition comparison. Raw extracts and diff scratch files are in this directory (`*.raw.txt`, `*.lay.txt`, `n08/n15/n21.txt`).
Nothing from asn1core or any other implementation was read.

Citation form: `X.690 §a.b.c ¶n` where ¶n is the nth sentence as numbered by `itu_read`; where a clause has
lettered items (8.23.9) the item letter is used. All X.690 and X.680 references are the 02/2021 edition unless stated.
Quotes are at most 15 words.

Conventions used in the tables:
- `STD` = the standard states it. `DERIVED` = follows from a chain of clauses, chain shown. `POLICY` = the standard is
  silent; the row is a recommendation for the crate, not a rule of the standard.
- Error classes (crate-neutral names, mine): `FORM` (primitive/constructed or inner tag wrong), `ALPHA` (character not
  in the type's alphabet), `CODING` (octets are not a well-formed character coding: UTF-8, UCS-2, UCS-4),
  `TSYNTAX` (time string does not match the grammar), `TRANGE` (grammar ok, a field is out of range),
  `DERFORM` (valid BER, forbidden by DER/CER clause 10/11), `LEN` (length form).
- "decoder" behaviour: X.690 never states what a receiver does with invalid input. The only receiver statements are
  X.690 §7.3 (receivers who claim BER conformance support all alternatives), X.690 §8.23.6 ¶2 (same for strings) and the
  Summary ("implicit ... also used for decoding"). Everything labelled `reject` below is therefore DERIVED from "the
  value is not in the type" or POLICY, as marked. X.690 §3.11 defines a receiver as an implementation decoding the octets.

---------------------------------------------------------------------------------------------------------------------

## 1. Obligations table

Tags (X.680 Table 1 / Table 8 / §38.1.2 / §38.4 / §46.3 / §47.3): UTF8String 12 (0x0C), TIME 14 (0x0E),
NumericString 18 (0x12), PrintableString 19 (0x13), TeletexString/T61String 20 (0x14), VideotexString 21 (0x15),
IA5String 22 (0x16), UTCTime 23 (0x17), GeneralizedTime 24 (0x18), GraphicString 25 (0x19), VisibleString/ISO646String
26 (0x1A), GeneralString 27 (0x1B), UniversalString 28 (0x1C), CHARACTER STRING 29 (0x1D), BMPString 30 (0x1E),
DATE 31, TIME-OF-DAY 32, DATE-TIME 33, DURATION 34. Tags 31..34 need the high-tag-number identifier (X.690 §8.1.2.4):
DATE `1F 1F`, TIME-OF-DAY `1F 20`, DATE-TIME `1F 21`, DURATION `1F 22` (universal class, primitive; bit 6 per §8.1.2.5).
Constructed form sets bit 6: 0x20 added to the one-octet identifiers (e.g. 0x3A for VisibleString).

### 1.1 Restricted character strings (X.690 §8.23, §9.2, §10.2, §11.4)

| ID | clause and sentence | rule | encode | decode in BER | decode in DER | error class |
|---|---|---|---|---|---|---|
| S01 | X.690 §8.23.3 ¶1 | Each restricted string is encoded as if declared `[UNIVERSAL x] IMPLICIT OCTET STRING`, x = the string's own universal tag. | identifier = string's tag (primitive) | accept primitive or constructed with the string's own tag | same, primitive only (S05) | FORM |
| S02 | X.690 §8.23.3 ¶2, §8.23.4 ¶1, §8.23.5 ¶1 | The octet-string value is defined by §8.23.4/§8.23.5: NumericString and PrintableString use the same octets as VisibleString with the same characters; other 8-bit types use ISO/IEC 2022 8-bit codings per ISO/IEC 2375 registrations. | for Numeric/Printable/Visible/IA5 each character is one octet equal to its ISO 646 cell value (cell ranges in X.680 §43.7/§43.8, section 2 below) | octets outside the type's alphabet are not a value of the type (see S10) | same | ALPHA |
| S03 | X.690 §8.23.2 ¶1 | Each data value is encoded independently of other values of the type. | no cross-value state (no carry-over of designations between strings) | reset assumed designations at each string start | same | - |
| S04 | X.690 §8.23.6 ¶1-2, §7.3 ¶1 | Constructed forms (definite and indefinite length) are alternatives of the sender; receivers must handle all permitted forms. The example in §8.23.5.4 shows primitive, constructed definite and constructed indefinite. | primitive by default | accept constructed; segments handled per S06 | not applicable (S05) | FORM |
| S05 | X.690 §10.2 ¶1; §10.1 ¶1; §7.4 | DER: constructed form shall not be used for bit string, octet string and restricted character string types; definite length, minimal octets. | primitive, definite, minimal length | accept | reject constructed (STD for the encoder, DERIVED for the decoder via §7.4: no alternative encodings in DER) | DERFORM |
| S06 | X.690 §8.7.3.1 ¶2, §8.7.3.2 ¶1-2 and Notes 1-2, via §8.23.3 | Constructed form = zero or more segment encodings, each an OCTET STRING encoding (universal 4), possibly itself constructed (recursion). No significance on segment boundaries. Empty segments allowed (§8.7.3.1 NOTE). | n/a in DER | segments carry tag 0x04 (not the string tag); concatenate all leaf segment octets FIRST, then validate the whole value. A segment boundary may fall inside a multi-octet character (UTF-8, BMP, UCS-4). | n/a | FORM (wrong inner tag, DERIVED from Note 2), CODING |
| S07 | X.690 §9.2 ¶1-2 | CER (not DER) chunking: value over 1000 contents octets is constructed, fragments of exactly 1000 octets, last 1..1000, primitive fragments. | CER only | accept (it is BER) | n/a, CER-only; a DER decoder must not accept it (S05) | DERFORM |
| S08 | X.690 §8.1.3.2 a), §8.1.3.6, §8.1.5 | Primitive uses definite length; constructed may use definite or indefinite, EOC `00 00` follows contents for indefinite. (Length clauses belong to another slice; listed because strings depend on them.) | - | accept both | definite, minimal (§10.1) | LEN |
| S09 | X.690 §8.23.5.1 ¶1, §8.23.5.3 ¶1, Table 3 | Escape sequences are allowed only for TeletexString, VideotexString, GraphicString, GeneralString, and only those of the registrations used to define the type. | NumericString, PrintableString, VisibleString, IA5String: no ESC-based designation ever | for the Yes types, ESC sequences are part of the octets and must be tolerated (Table 3 "Yes"); the crate need not interpret them | same | ALPHA if ESC appears in a "No" type (DERIVED: ESC 0x1B is not in the alphabet, section 2) |
| S10 | X.680 §41 intro, §41.2, §41.4, §43.5-43.8 | A value of the type is a sequence of characters of the type's alphabet. Neither X.680 nor X.690 says what a receiver does with an octet outside it. | encoder: reject a value containing a character outside the alphabet | not stated. DERIVED: octets outside the alphabet do not denote a value of the type. POLICY: reject (class ALPHA). | same | ALPHA |
| S11 | X.690 §8.23.5.2 ¶1, Table 3 | At the start of each 8-bit string certain registrations are assumed designated: Teletex G0=102, C0=106, C1=107; Videotex G0=102, C0=1, C1=73; Graphic G0=6, no C; General G0=6, C0=1; Numeric/Printable/Visible G0=6, no C; IA5 G0=6, C0=1. | - | octets 0x00-0x1F are controls only where a C0 is assumed (Table 3), see section 2 | same | ALPHA |
| S12 | X.690 §8.23.5.4 ¶1 | Announcers shall not be used unless explicitly permitted by the user of ASN.1. | do not emit | POLICY: tolerate as opaque octets in the 8-bit types that allow escapes | same | - |
| S13 | X.690 §8.23.7 ¶1-3 | UniversalString: ISO/IEC 10646 4-octet canonical form (13.2 of 10646), no signature (no BOM), control functions allowed subject to §8.23.9. | 4 octets per character, big-endian code point | length of the reassembled value must be a multiple of 4 (DERIVED from "4-octet form"); no BOM allowed | same | CODING |
| S14 | X.690 §8.23.8 ¶1-3 | BMPString: ISO/IEC 10646 2-octet BMP form (13.1 of 10646), no signature, controls per §8.23.9. | 2 octets per character, big-endian | reassembled length must be even (DERIVED); a leading FE FF / FF FE is a signature, forbidden (¶2) | same | CODING |
| S15 | X.680 §41.15 | BMPString contains only characters of the Basic Multilingual Plane (first 64K-2 cells, less cells whose encoding addresses characters outside the BMP). | reject code points D800-DFFF and U+10000 and above | surrogates (D800-DFFF) are not BMPString characters (STD text; D800-DFFF identification is DERIVED from "cells ... used to address characters outside the BMP") | same | CODING |
| S16 | X.690 §8.23.9 a)-f) | For BMP/Universal: ISO/IEC 2022 announcers, designating and invoking escapes, SI/SO/LS3, 10646 §16.3 subset-identifying sequences, §16.5 escapes shall not be used; C0/C1 controls are allowed; coding stays in 10646. | do not emit | POLICY: treat as ordinary code points; the standard gives no byte-level pattern to detect "escape sequence" in UCS-2/UCS-4 other than the code points themselves | same | - |
| S17 | X.690 §8.23.10 ¶1 | UTF8String: octets as specified in ISO/IEC 10646 Annex D. | UTF-8 of the character string | validity of the coding is decided by ISO/IEC 10646 Annex D (NOT in the itulib corpus; see Sections 5 and 6) | same | CODING |
| S18 | X.690 §8.23.10 ¶2 | Announcers and escape sequences not used; each character in the smallest number of octets available for it. | shortest form only | overlong forms are not conforming (STD: "smallest number of octets") - reject | same | CODING |
| S19 | X.680 §41.16, §41.6 | UTF8String is synonymous with UniversalString at the abstract level; UniversalString holds any character allowed by 10646. | - | the abstract character set is "any character allowed by 10646"; this text names no surrogate, noncharacter or upper (U+10FFFF) limit | same | CODING |
| S20 | X.690 §8.24.1 ¶1, §8.24.2 ¶1; X.680 §44.2, §44.5 | CHARACTER STRING (unrestricted, UNIVERSAL 29) is encoded as the BER (or CER/DER, §11 lead-in) encoding of the associated SEQUENCE in X.680 §44.5; string-value OCTET STRING holds the octets in the identified transfer syntax. | encode as SEQUENCE { identification CHOICE {...}, string-value OCTET STRING } with no data-value-descriptor | decode the SEQUENCE; do not interpret string-value | same | FORM |
| S21 | X.690 §11.4 ¶1-2 | For GeneralString and all registry-defined types, an escape sequence to designate/invoke is generated only when the entry is not already designated in G0-G3/C0/C1, and always into the smallest-numbered G or C set that can take it. | encoder rule (needs the ISO register, not in the corpus) | not an obligation on receivers | same (DER/CER inherit it, §11) | - |
| S22 | X.690 §11 lead-in | For CER/DER, "shall be the BER encoding" is read as "the CER or DER encoding". | - | - | applies to CHARACTER STRING contents | - |

### 1.2 Useful time types UTCTime and GeneralizedTime (X.690 §8.25, §11.7, §11.8; X.680 §46, §47)

| ID | clause and sentence | rule | encode | decode in BER | decode in DER | error class |
|---|---|---|---|---|---|---|
| T01 | X.690 §8.25 ¶1; X.680 §46.3, §47.3 | UTCTime = `[UNIVERSAL 23] IMPLICIT VisibleString`, GeneralizedTime = `[UNIVERSAL 24] IMPLICIT VisibleString`, restricted to the grammars below; encoded as those definitions. | tag 0x17 / 0x18, primitive | accept primitive; constructed is DERIVED allowed (8.25 -> X.680 46.3 -> 8.23.3 -> 8.7) | primitive (DERIVED via §10.2, see conflict C4) | FORM / DERFORM |
| T02 | X.680 §47.3 a), b), c) | UTCTime grammar: `YYMMDD` then `hhmm` or `hhmmss`, then `Z` or (`+`/`-`)`hhmm`. DD 01..31, hh 00..23, mm 00..59, ss 00..59. All digits ASCII. | DER: T05 | accept both precisions, both zone forms; offset is 4 digits always | see T05 | TSYNTAX, TRANGE |
| T03 | X.680 §47.3 closing paragraph | If a zone offset t2 is present the instant is UTC = t1 - t2; offset positive means local ahead of UTC (per §46.2 NOTE for GT, same sign convention). | - | decoder may convert | n/a (DER has Z only) | - |
| T04 | X.680 §46.2, §46.3 a), b), c) | GeneralizedTime grammar (BER): `YYYYMMDD` + time of day + zone. Time of day: `hh`, `hhmm` or `hhmmss`, each optionally followed by a fraction (decimal sign `,` or `.`, then digits, any count) of the last present unit. Zone: absent (local), `Z`, or (`+`/`-`)`hh` or `hhmm`. Midnight at the end of a day is excluded (hh = 24 is not a value). | DER: T06 | accept all combinations | see T06 | TSYNTAX, TRANGE |
| T05 | X.690 §11.8.1, §11.8.2, §11.8.3 | DER/CER UTCTime: terminates with `Z` (no offset); seconds always present; midnight is `YYMMDD000000Z` where YYMMDD is the day following the midnight. | exactly `YYMMDDhhmmssZ`, 13 octets | accept the BER grammar | accept only `YYMMDDhhmmssZ` | DERFORM |
| T06 | X.690 §11.7.1, §11.7.2, §11.7.3, §11.7.4, §11.7.5 | DER/CER GeneralizedTime: ends with `Z`; seconds present; fraction (if any) has no trailing zero, and a zero fraction is wholly omitted with its decimal point; decimal sign is `.`; midnight is `YYYYMMDD000000Z` (next day). | `YYYYMMDDhhmmss[.f...f]Z`, last f in 1..9 | accept the BER grammar | accept only that form | DERFORM |
| T07 | X.690 §11.7.5 example, §11.8.5 | `19920520240000Z` and `920520240000Z` (hh=24) are invalid; `19920622123421.0Z`, `19920722132100.30Z`, `9207221321Z` invalid in DER/CER. | - | 24:00 rejected by X.680 §46.2 / §47.3 (hh 00..23) also in BER | reject | TRANGE / DERFORM |
| T08 | X.680 §47.3 vs X.690 (searched) | Neither standard assigns a century to the two-digit year (see section 3). | - | decoder must take a pivot from the application or profile | same | - |
| T09 | X.680 §46.3 NOTE 3; §46.1 NOTE 2 | GeneralizedTime keeps its original semantics for compatibility; TIME is recommended for new specifications. Fractions of a different digit count are the same value in GeneralizedTime. | - | value equality ignores accuracy for GT/UTCTime (the TIME type distinguishes accuracy: X.680 §J.2.9 NOTE) | - | - |
| T10 | X.680 Annex C, C.3.2.5 f) | For value mapping, each GeneralizedTime/UTCTime/TIME/TIME-OF-DAY/DATE/DATE-TIME/DURATION value is replaced by its DER/CER-canonical string (X.690 §11.7-11.9). | - | canonicalise before comparing values | - | - |
| T11 | X.680 §47.2 b) / §47.3 | UTCTime precision is minute or second; GT precision is hour, minute, second or fraction of any of them. | - | - | - | - |

### 1.3 TIME and the useful time types TIME, DATE, TIME-OF-DAY, DATE-TIME, DURATION (X.690 §8.26, §11.9; X.680 §38)

| ID | clause and sentence | rule | encode | decode in BER | decode in DER | error class |
|---|---|---|---|---|---|---|
| M01 | X.690 §8.26.1 NOTE; X.680 §38.1.2, §38.4.1-38.4.4 | TIME is UNIVERSAL 14; DATE 31, TIME-OF-DAY 32, DATE-TIME 33, DURATION 34 are subtypes of TIME with their own tags. | tag per type | accept the tag of the type the schema names | same | FORM |
| M02 | X.690 §8.26.1.1, §8.26.2.1, §8.26.3.1, §8.26.4.1, §8.26.5.1 | All five are primitive. | primitive only | reject constructed (STD: the encoding "shall be primitive") | same | FORM |
| M03 | X.690 §8.26.1.2 | TIME contents = UTF-8 of the X.680 value notation with the initial and final quotation marks removed. Nothing else removed (hyphens, colons, T, Z, P stay). | - | contents are ASCII (X.680 §12.16 simplestring: codes 32..126) | same | TSYNTAX |
| M04 | X.690 §8.26.2.2 | DATE: value notation minus quotes and all HYPHEN-MINUS. DATE = `Basic=Date Date=YMD Year=Basic` (X.680 §38.4.1): `YYYY-MM-DD` -> `YYYYMMDD`. | 8 digits | accept 8 digits | same | TSYNTAX |
| M05 | X.690 §8.26.3.2 | TIME-OF-DAY: minus quotes and all COLON. Type is `Basic=Time Time=HMS Local-or-UTC=L` (X.680 §38.4.2): `hh:mm:ss` -> `hhmmss`; no zone designator. X.680 §38.4.2 NOTE allows 24:00:00. | 6 digits | accept 6 digits | same | TSYNTAX |
| M06 | X.690 §8.26.4.2 | DATE-TIME: minus quotes, all HYPHEN-MINUS, all COLON and the letter T. `YYYY-MM-DDThh:mm:ss` -> `YYYYMMDDhhmmss`. X.680 §38.4.3 NOTE allows T24:00:00. | 14 digits | accept 14 digits | same | TSYNTAX |
| M07 | X.690 §8.26.5.2 | DURATION: minus quotes and the letter P. `P1Y2M3DT4H5M6S` -> `1Y2M3DT4H5M6S`. T is kept. | - | accept the X.680 §38.3.6 grammar after P removal | same | TSYNTAX |
| M08 | X.690 §11.9.1 a) | Canonical (DER/CER): commas used as decimal signs become full stop. | `.` | accept `,` and `.` | reject `,` | DERFORM |
| M09 | X.690 §11.9.1 b) | Canonical: minutes digits of a time difference that is a whole number of hours are removed (`+05:00` -> `+05`). Applies to TIME; DATE-TIME and TIME-OF-DAY carry no difference (Local-or-UTC=L). | `+hh` | accept `+hh` and `+hh:mm` | reject `+hh:00` | DERFORM |
| M10 | X.690 §11.9.1 c) | Canonical: in an interval with start and end point, an end-point time difference equal to the start's is removed. | - | accept either | reject duplicate | DERFORM |
| M11 | X.690 §11.9.1 d) | Canonical: in a duration (and duration inside an interval), remove all zero components except the least significant one present (`P0Y29M0D` -> `P29M0D`). | applied before 8.26.5.2 removes P | accept zero components | reject redundant leading zero components | DERFORM |
| M12 | X.690 §11.9.2 | The canonical value notation is then encoded per §8.26. | - | - | - | - |
| M13 | X.680 §38.3.6.11, 38.3.6.12, 38.3.6.13 | Duration designation: integral part has no leading zeros unless it is the single digit 0; a fractional part is decimal sign + one or more digits; a designation with a fractional part has no following designation. | - | enforce | enforce | TSYNTAX |
| M14 | X.680 §38.3.4 NOTE 1, §38.3.6.14 | Different accuracy = different abstract value (`P29M` vs `P29M0D`). | keep digits | do not normalise accuracy | same | - |
| M15 | X.680 Table 6 (Year setting), §38.4.1 | DATE/DATE-TIME use `Year=Basic`: years 1582..9999, four digits (`[YYYY]` Table 7). | - | reject years outside 1582..9999 for DATE/DATE-TIME (DERIVED) | same | TRANGE |

TIME (UNIVERSAL 14) in general can carry every abstract value of X.680 Table 6/7 (intervals, recurrence, week dates,
centuries, large years). The full grammar is out of scope for this slice; M03 is the only encoding rule for a general TIME.

---------------------------------------------------------------------------------------------------------------------

## 2. Alphabet tables

Method: X.680 Tables 8, 9, 10 and the canonical-order text §43.5-43.8 were extracted with `pdftotext -raw` and again
with `-layout`; Table 10 was additionally dumped as UTF-8 bytes (`od -c`) to confirm that the graphics are plain ASCII
(0x27 apostrophe, no typographic quote) and that the names agree. Table 3 of X.690 was extracted with `-raw`.
No image rendering tool (pdftoppm, PyMuPDF) is installed here, so the PDF pages were not inspected visually; the text layer is the
evidence. Cross-checks inside the document: Table 10 lists 12 punctuation/space rows + 62 alphanumerics = 74, matching
§43.6 "precisely 74"; §43.5 "precisely 11" matches Table 9; §43.7 "precisely 95" matches cells 2/0-7/14; §43.8 "precisely
128". `-layout` extraction scrambles the row order of Table 8 and the Table 10 first column, so Table 8 below is taken from `-raw`
and its tag numbers were cross-checked with X.680 Table 1 (UNIVERSAL 18-22, 25-30 character strings; 12 UTF8String).

### 2.1 NumericString (X.680 §41.2 Table 9, §43.5) - verified, 11 characters

| code (hex) | character |
|---|---|
| 0x20 | SPACE |
| 0x30-0x39 | digits 0-9 |

Everything else is outside the alphabet. Codes are ASCII; X.690 §8.23.4 makes the contents octets the VisibleString octets.

### 2.2 PrintableString (X.680 §41.4 Table 10, §43.6) - verified, 74 characters

| code (hex) | character(s) |
|---|---|
| 0x20 | SPACE |
| 0x27 | APOSTROPHE `'` |
| 0x28, 0x29 | LEFT / RIGHT PARENTHESIS `(` `)` |
| 0x2B | PLUS SIGN |
| 0x2C | COMMA |
| 0x2D | HYPHEN-MINUS |
| 0x2E | FULL STOP |
| 0x2F | SOLIDUS |
| 0x30-0x39 | digits |
| 0x3A | COLON |
| 0x3D | EQUALS SIGN |
| 0x3F | QUESTION MARK |
| 0x41-0x5A | A-Z |
| 0x61-0x7A | a-z |

The hex values are my mapping of the named graphics to ASCII; the standard gives names and graphics, plus §43.6 states the order
is "the same as" the BASIC LATIN collection of 10646 (that sentence confirms the ASCII correspondence). Outside the
alphabet, notably: `!` 0x21, `"` 0x22, `#` `$` `%` `&` (0x23-0x26), `*` 0x2A, `;` 0x3B, `<` 0x3C, `>` 0x3E, `@` 0x40, `[` to backtick
(0x5B-0x60), `{` to DEL (0x7B-0x7F), `_` 0x5F, all controls, all octets >= 0x80.

### 2.3 VisibleString / ISO646String (X.680 Table 8 "6 + SPACE", §43.7) - verified text, 95 characters

0x20-0x7E. Source: §43.7 "cells 2/0-7/14 of the ISO/IEC 646 code table", count 95. DEL (0x7F) and all C0/C1 controls are excluded
(Table 8 lists no DELETE and no C set; X.690 Table 3 assumes no C0/C1 for VisibleString).

### 2.4 IA5String (X.680 Table 8 "1, 6 + SPACE + DELETE", §43.8; X.690 Table 3) - verified text, 128 characters

0x00-0x7F: C0 set (register entry 1, cells 0/0-1/15), the graphics of entry 6, SPACE, DELETE. §43.8 states the canonical order is the
ISO/IEC 646 encoding itself and the set holds 128 characters. 0x80 and above are outside. X.680 §41.14 confirms entry 1 in columns
0-1 and entry 6 in columns 2-7 (the code table), consistent with 0x00-0x7F.

### 2.5 TeletexString/T61String, VideotexString, GraphicString, GeneralString - NOT enumerable from the corpus

X.680 Table 8 defines them by ISO register entry numbers: Teletex = 6, 87, 102, 103, 106, 107, 126, 144, 150, 153, 156, 164, 165, 168 + SPACE
+ DELETE; Videotex = 1, 13, 72, 73, 87, 89, 102, 108, 126, 128, 129, 144, 150, 153, 164, 165, 168 + SPACE + DELETE; GraphicString = all G sets + SPACE; GeneralString
= all G and all C sets + SPACE + DELETE. The register (ISO/IEC 2375 / the ISO International Register) is not in the corpus, so no code-point table can
be given. What X.690 does give (Table 3, §8.23.5.2) is the assumed starting state and that escapes are allowed (section 1.1 S09, S11).
Implementation consequence: treat these four as opaque 8-bit octet strings (cannot be validated per the standard text available);
the one statically checkable thing is that GraphicString has no assumed C set and GeneralString has C0 = register 1.

### 2.6 UniversalString, BMPString, UTF8String

| type | unit | set (standard text) | verification |
|---|---|---|---|
| UniversalString | 4 octets, big-endian | "any of the characters allowed by ISO/IEC 10646" (X.680 §41.6) | text read; 10646 not in corpus |
| BMPString | 2 octets, big-endian | the first 64K-2 cells of 10646 less the cells used to address characters outside the BMP (X.680 §41.15); D800-DFFF are those cells | the two cells excluded by "64K-2" are not named; I infer U+FFFE and U+FFFF (noncharacters) but that is NOT stated in X.680 or X.690; see section 6 |
| UTF8String | 1-4 octets | same abstract set as UniversalString (X.680 §41.16), octets per 10646 Annex D, shortest form (X.690 §8.23.10) | Annex D not in corpus |

---------------------------------------------------------------------------------------------------------------------

## 3. UTCTime and GeneralizedTime: exact grammars, century, conflicts

### 3.1 BER (X.680 §47.3) UTCTime

```
UTCTime  := YY MM DD hh mm [ ss ] ( "Z" | ("+"|"-") hh mm )
YY MM DD hh mm ss mm  = two ASCII digits each; DD 01..31, hh 00..23, mm 00..59, ss 00..59   (X.680 §47.3 a), b))
```
Seconds optional (§47.3 b) 1) vs 2)); zone mandatory (Z, or sign and exactly four digits). The standard gives ranges for DD (01 to 31)
and for hh/mm/ss. For the month it gives only "counting January as 01"; it does not give the offset's hh/mm ranges. Whether DD=31 is
valid in a 30-day month, or 0229 in a common year, is not stated beyond §47.2 a) "calendar date" (DERIVED: must be a real date).

### 3.2 BER (X.680 §46.2, §46.3) GeneralizedTime

```
GT    := YYYY MM DD  Time  [ Zone ]
Time  := hh [Frac] | hh mm [Frac] | hh mm ss [Frac]
Frac  := ("," | ".") digit+                       -- decimal fraction of the LAST unit present
Zone  := "Z" | ("+"|"-") hh [ mm ]                -- absent = local time; minutes may be omitted for whole hours (§46.3 c))
```
Midnight at end of day is excluded (§46.2 a), b)): hh = 24 not allowed. No range is stated for ss (leap second 60) or for the offset digits; no limit is stated on the number of fraction digits ("any number of decimal places"). A bare decimal sign with no digit is not shown by any example (ISO 8601 not available).

### 3.3 DER/CER restriction

```
UTCTime DER := YY MM DD hh mm ss "Z"                                     (13 octets)     X.690 §11.8.1-§11.8.2
GT DER      := YYYY MM DD hh mm ss [ "." digit* nonzero-last ] "Z"       (15, or 17.. octets)  X.690 §11.7.1-§11.7.4
```
Zero fraction: no `.` and no digits (§11.7.3). Midnight rule §11.7.5 / §11.8.3 is consistent with hh in 00..23. Examples that are valid/invalid are
in X.690 §11.7.5 and §11.8.4-5; vectors below reproduce them.

### 3.4 Century rule for two-digit years

- X.680 §47.3 a): "YY is the two low-order digits of the Christian year" - it does not map YY to a century. Examples 1 and 2 of §47.3 show YY=82 meaning 1982 and YY=01 meaning 2001, but they only illustrate; no pivot is stated.
- X.690: no sentence found (grep for century, 1950, 2050, pivot, two low-order in X.680, X.690 2008/2021: none outside the time-type century notation `C`, which concerns TIME, not UTCTime).
- Therefore: the standard states NO century rule. The 49/50 pivot is a PKIX profile rule, outside ITU-T: RFC 5280 §4.1.2.5.1 ¶3 (read with rfclib): YY >= 50 is 19YY, otherwise 20YY. A general BER crate must take the pivot as a parameter, defaulting to that profile rule only as an explicit choice.
- Not a conflict between X.680 and X.690 (X.690 is silent on the matter), but a gap.

### 3.5 Places where texts appear to conflict or are ambiguous

- C1. X.680 §46.3 permits local time, offsets, hour/minute precision, comma; X.690 §11.7 narrows DER to `Z`, seconds present, `.` only. Not a conflict: §46.3 NOTE 3 explains it is for backward compatibility and the canonical rules cover only UTC. A decoder in DER mode must reject BER-only forms.
- C2. X.690 §11.7.5/§11.8.3 call the midnight form "Midnight (GMT)" and use the day after; X.680 §46.2/§47.3 exclude hh = 24. Consistent.
- C3. X.690 §8.23.5 (2008) excluded only UniversalString and BMPString while §8.23.10 (2008) described UTF8String in UTF-8: an internal gap, fixed in 2021 by adding UTF8String to §8.23.5 (section 4).
- C4. Whether UTCTime/GeneralizedTime may be constructed in BER and must be primitive in DER. X.690 §10.2 names "restricted character string types"; X.680 §41's list of restricted types does not include UTCTime/GeneralizedTime. They become strings by X.690 §8.25 ("as if replaced by their definitions", i.e. IMPLICIT VisibleString) -> §8.23.3 -> OCTET STRING. By that chain BER may use constructed 0x37/0x38 and DER must not, but no sentence says so for these two types directly. Treated as DERIVED; flag for the crate's design review.
- C5. X.690 §8.23.6 begins "The above example illustrates", yet the example appears under §8.23.5.4 and §8.23.6 itself never says "constructed". Cross-references (§9.2 and §10.2 "Contrast with 8.23.6") confirm it is the constructed-form permission.
- C6. X.680 Table 7, "Day component" row prints the property as `"Year=YMD"`, `"Year=YD"`, `"Year=YWD"`; every other place (Table 6, §38.4.1) names the property `Date`. Almost certainly a typographical error in the standard. Not an encoding issue.
- C7. X.690 §8.26.3.2/§8.26.4.2 remove colons but the DATE-TIME example in X.680 Table 7 prints `[Thh]`; the T is removed by §8.26.4.2, so contents have no T. Consistent.
- C8. X.680 §41.15 "64K-2 cells" does not name the two excluded cells in addition to the surrogate area. Ambiguity, see section 6.

---------------------------------------------------------------------------------------------------------------------

## 4. Edition differences

Method: `itu_compare` on each clause (it reports only the clause heading for clauses with subclauses, so is not sufficient), then a sentence-level
`diff` of `pdftotext -raw` output for X.690 §8.23 through §12 with whitespace and running headers normalised, for 11/2008 vs 02/2021 and 08/2015 vs 02/2021.

| # | clause | 2008-11 | 2021-02 | effect |
|---|---|---|---|---|
| E1 | X.690 §8.23.5 ¶1 | restricted strings "apart from UniversalString and BMPString" use ISO/IEC 2022 8-bit coding | "apart from UniversalString, UTF8String and BMPString" | none on a conformant UTF8String encoder (§8.23.10 already said Annex D); removes the 2008 internal contradiction. A 2008-based decoder that routed UTF8String through ISO 2022 rules was wrong. |
| E2 | all other clauses in the slice (§8.23.1-8.23.10 apart from E1, Table 3, §8.24, §8.25, §8.26, §9.2, §10.2, §11.4, §11.7, §11.8, §11.9) | - | identical wording | none. The TIME material (§8.26, §11.9) is already in the 2008 edition. Cross-references changed only in typography ("ITU-T Rec. X.680" -> "Rec. ITU-T X.680"). |
| E3 | 08/2015 vs 02/2021 for the same clauses | identical | identical | none |

X.690 Amendment 2 (06/2006), "Time type support", to the 07/2002 edition is registered in `itu_info` but its text is not in the corpus and the 2002 edition is not
either; thus what 2002 said about UTCTime/GeneralizedTime cannot be compared. X.680 only exists as 02/2021 in the corpus (`the fetched corpus`), so the X.680 2008 vs 2021 comparison
the task requested CANNOT be done with the available sources. X.680 §46 NOTE 1 states the time-type text was reworded after earlier editions but "the technical content is unchanged from the first version"; that statement is the only edition evidence available for §46 (not verified against 2008). Differences that change an encoding or validity rule: E1 only, and it changes no octet.

---------------------------------------------------------------------------------------------------------------------

## 5. Test vectors

Notation: bytes in hex, no spaces required. Mode: BER = BER decoder, DER = DER decoder (strict), both = same result in both. Expected: value or `ERR <class>`.
Provenance: `X` = exactly the form from the standard; `H` = constructed by hand (construction shown in the row's note).
Framing recipe used everywhere: identifier + length + ASCII of the string; length is the number of contents octets in hex.

### 5.1 Strings: forms (from X.690 §8.23.5.4 example)

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| V01 | `1A 05 4A6F6E6573` | both | VisibleString "Jones" | from X.690 §8.23.5.4 (primitive example) |
| V02 | `3A 09 04 03 4A6F6E 04 02 6573` | BER | "Jones" | from X.690 §8.23.5.4 (constructed, definite) |
| V02d | `3A 09 04 03 4A6F6E 04 02 6573` | DER | ERR DERFORM (X.690 §10.2) | from X.690 §8.23.5.4 plus §10.2 ¶1 |
| V03 | `3A 80 04 03 4A6F6E 04 02 6573 00 00` | BER | "Jones" | from X.690 §8.23.5.4 (constructed, indefinite) |
| V03d | `3A 80 04 03 4A6F6E 04 02 6573 00 00` | DER | ERR DERFORM (indefinite and constructed) | from X.690 §8.23.5.4 plus §10.1-§10.2 |
| V04 | `3A 80 24 80 04 01 4A 00 00 04 04 6F6E6573 00 00` | BER | "Jones" (nested constructed segment) | constructed by hand from X.690 §8.7.3.2 NOTE 1 and §8.23.3: outer 3A 80; segment 24 80 {04 01 'J'} 00 00; segment 04 04 "ones"; EOC |
| V05 | `3A 05 1A 03 4A6F6E` | BER | ERR FORM (inner segment tag must be 04) | constructed by hand from X.690 §8.7.3.2 NOTE 2: constructed VisibleString whose only segment is a VisibleString tag 1A |
| V06 | `13 00` | both | PrintableString "" | constructed by hand from X.690 §8.7.2 (zero contents octets) |
| V07 | `3A 05 04 00 04 01 4A` | BER | "J" (an empty first segment is permitted) | constructed by hand from X.690 §8.7.3.1 NOTE: 04 00 (empty) + 04 01 4A |

### 5.2 PrintableString (alphabet X.680 §41.4; boundaries per section 2.2)

Every row `13 01 XX` (single character) unless stated. All rows constructed by hand from X.680 §41.4 Table 10 (identifier 0x13, length 01, the octet).

| ID | hex input | mode | expected |
|---|---|---|---|
| P01 | `13 1C 41424358595A61626378797A30313839202728292B2C2D2E2F3A3D3F` | both | valid: "ABCXYZabcxyz0189 '()+,-./:=?" (28 chars: all non-alphanumeric members once) |
| P02 | `13 01 20`, `13 01 27`, `13 01 28`, `13 01 29`, `13 01 2B`, `13 01 2C`, `13 01 2D`, `13 01 2E`, `13 01 2F`, `13 01 3A`, `13 01 3D`, `13 01 3F` | both | valid each |
| P03 | `13 01 41`, `13 01 5A`, `13 01 61`, `13 01 7A`, `13 01 30`, `13 01 39` | both | valid each (A, Z, a, z, 0, 9) |
| P04 | `13 03 414042` ("A@B") | both | ERR ALPHA (0x40 `@` not in Table 10) |
| P05 | `13 03 612A62` ("a*b") | both | ERR ALPHA (0x2A `*`) |
| P06 | `13 03 612662` ("a&b") | both | ERR ALPHA (0x26 `&`) |
| P07 | `13 01 5F` | both | ERR ALPHA (underscore) |
| P08 | `13 01 21`, `13 01 22`, `13 01 23`, `13 01 24`, `13 01 25` | both | ERR ALPHA each |
| P09 | `13 01 3B`, `13 01 3C`, `13 01 3E` | both | ERR ALPHA (neighbours of valid `:` `=` `?`) |
| P10 | `13 01 5B`, `13 01 60`, `13 01 7B`, `13 01 7F`, `13 01 1F`, `13 01 00`, `13 01 80`, `13 01 FF` | both | ERR ALPHA each |

Decoder behaviour on ERR ALPHA is POLICY (S10): the standard states no receiver action. Exhaustive test: for b in 0..255 decode `13 01 b`; accepted set must equal the table in 2.2 (74 values).

### 5.3 NumericString, VisibleString, IA5String

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| N01 | `12 0B 30313233343536373839 20` (written `12 0B 3031323334353637383920`) | both | valid "0123456789 " | by hand from X.680 §41.2 Table 9 |
| N02 | `12 03 313261` | both | ERR ALPHA (0x61 `a`) | by hand from X.680 §41.2 |
| N03 | `12 01 2D`, `12 01 2F`, `12 01 3A`, `12 01 1F`, `12 01 80` | both | ERR ALPHA each (neighbours of 0x20/0x30/0x39) | by hand from X.680 §41.2 |
| N04 | `12 00` | both | valid empty | by hand from X.690 §8.7.2 |
| W01 | `1A 01 20`, `1A 01 7E` | both | valid (space, tilde: ends of 2/0-7/14) | by hand from X.680 §43.7 |
| W02 | `1A 01 7F`, `1A 01 1F`, `1A 01 80` | both | ERR ALPHA (DEL, control, high bit) | by hand from X.680 §43.7 / Table 8 (no DELETE) |
| I01 | `16 01 00` | both | valid U+0000 | by hand from X.680 §43.8 (128 characters) |
| I02 | `16 01 7F` | both | valid (DELETE is in IA5String) | by hand from X.680 Table 8 "1, 6 + SPACE + DELETE", §43.8 |
| I03 | `16 01 80` | both | ERR ALPHA | by hand from X.680 §43.8 |
| I04 | `16 01 FF` | both | ERR ALPHA | by hand from X.680 §43.8 |

### 5.4 UTF8String (tag 0x0C)

X.680 §41.16 says UTF8String is the same abstract set as UniversalString ("any character allowed by 10646", §41.6) and X.690 §8.23.10 delegates
the octet form to ISO/IEC 10646 Annex D with the extra "smallest number of octets" rule. Only overlong forms are decided by text I have; surrogates and out-of-range scalars
are decided by Annex D / the Unicode reference, which are not in the corpus. Those rows are marked POLICY.

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| U01 | `0C 02 C3A9` | both | valid U+00E9 | by hand from X.690 §8.23.10 (2-octet UTF-8) |
| U02 | `0C 03 E282AC` | both | valid U+20AC | by hand from X.690 §8.23.10 |
| U03 | `0C 04 F09F9880` | both | valid U+1F600 | by hand from X.690 §8.23.10 |
| U04 | `0C 02 C0AF` | both | ERR CODING (overlong encoding of `/`; STD: X.690 §8.23.10 ¶2 smallest number of octets) | by hand from X.690 §8.23.10 ¶2: `/` = 0x2F needs 1 octet; C0 AF is the 2-octet form |
| U05 | `0C 03 E080AF` | both | ERR CODING (overlong 3-octet form of `/`) | by hand from X.690 §8.23.10 ¶2 |
| U06 | `0C 03 EDA080` | both | POLICY ERR CODING: U+D800 surrogate. Standard text gives no sentence; validity rests on ISO/IEC 10646 Annex D (not read) | by hand: U+D800 -> ED A0 80 |
| U07 | `0C 02 E282` | both | ERR CODING (truncated: lead E2 announces 3 octets, 2 present). Decided by Annex D / length consistency, not by a clause text I could read | by hand from U02 minus last octet |
| U08 | `0C 01 80` | both | ERR CODING (lone continuation) | by hand |
| U09 | `0C 04 F4908080` | both | POLICY ERR CODING (U+110000, above 10FFFF; limit not stated in X.680/X.690) | by hand |
| U10 | `2C 07 04 01 E2 04 02 82AC` | BER | valid U+20AC (segment boundary inside a character is allowed; reassemble first) | by hand from X.690 §8.7.3.1 ¶2 (no significance on boundaries) |
| U10d | same | DER | ERR DERFORM | X.690 §10.2 |

### 5.5 BMPString (tag 0x1E) and UniversalString (tag 0x1C)

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| B01 | `1E 02 0041` | both | valid "A" | by hand from X.690 §8.23.8 ¶1 |
| B02 | `1E 04 00480069` | both | valid "Hi" | by hand from X.690 §8.23.8 ¶1 |
| B03 | `1E 03 004100` | both | ERR CODING (odd number of octets; 2-octet form cannot parse; derived from §8.23.8 ¶1) | by hand from X.690 §8.23.8 ¶1 |
| B04 | `1E 02 D800` | both | ERR CODING (lone surrogate: not a BMP character per X.680 §41.15) | by hand from X.680 §41.15 |
| B05 | `1E 04 D83DDE00` | both | ERR CODING (UTF-16 pair for U+1F600: BMPString holds no characters outside the BMP, X.680 §41.15; the 2-octet BMP form is not UTF-16, X.690 §8.23.8) | by hand from X.680 §41.15 and X.690 §8.23.8 |
| B06 | `1E 04 FEFF0041` | both | ERR CODING (signature forbidden by X.690 §8.23.8 ¶2). Decoder note: U+FEFF is otherwise a character; the standard text forbids it as a signature at the start | by hand from X.690 §8.23.8 ¶2 |
| B07 | `3E 06 04 01 00 04 01 41` | BER | valid "A" (boundary inside a code unit; reassemble first) | by hand from X.690 §8.7.3.1 ¶2 |
| B07d | same | DER | ERR DERFORM | X.690 §10.2 |
| B08 | `1E 02 FFFE`, `1E 02 FFFF` | both | UNDECIDED by the text (see section 6, 64K-2); POLICY accept/reject must be chosen | by hand |
| C01 | `1C 04 00000041` | both | valid "A" | by hand from X.690 §8.23.7 ¶1 |
| C02 | `1C 03 000041` | both | ERR CODING (length not a multiple of 4) | by hand from X.690 §8.23.7 ¶1 |
| C03 | `1C 04 0000D800` | both | POLICY ERR CODING (surrogate code point; text silent) | by hand |
| C04 | `1C 04 0001F600` | both | valid U+1F600 (outside BMP is allowed in UniversalString, X.680 §41.6) | by hand from X.680 §41.6 |

### 5.6 UTCTime (tag 0x17) and GeneralizedTime (tag 0x18)

Hex of the string part is the plain ASCII. Rows list the ASCII value; the hex is `tag len ASCII` (computed with `od`).

| ID | string | full hex | mode | expected | provenance |
|---|---|---|---|---|---|
| UT01 | `920521000000Z` | `17 0D 3932303532313030303030305A` | both | valid, 1992-05-21T00:00:00Z (century by pivot) | by hand from X.690 §11.8.4 (string is the example) |
| UT02 | `920622123421Z` | `17 0D 3932303632323132333432315A` | both | valid | by hand from X.690 §11.8.4 |
| UT03 | `920722132100Z` | `17 0D ...` (`3932303732323133323130305A`, 13 octets) | both | valid | by hand from X.690 §11.8.4 |
| UT04 | `920520240000Z` | `17 0D 3932303532303234303030305A` | both | ERR TRANGE (hh=24; X.690 §11.8.5, X.680 §47.3 b) 00..23) | by hand from X.690 §11.8.5 |
| UT05 | `9207221321Z` | `17 0B 393230373232313332315A` | BER | valid (minutes precision, X.680 §47.3 b) 1)) | by hand from X.680 §47.3 b) 1) |
| UT05d | same | | DER | ERR DERFORM (seconds omitted, X.690 §11.8.5) | by hand from X.690 §11.8.5 |
| UT06 | `8201020700-0500` | `17 0F 383230313032303730302D30353030` | BER | valid; UTC = 1982-01-02T12:00 | by hand from X.680 §47.3 Example 1 |
| UT06d | same | | DER | ERR DERFORM (offset, X.690 §11.8.1) | by hand from X.690 §11.8.1 |
| UT07 | `0101021200Z` | `17 0B 303130313032313230305A` | BER | valid, YY=01 | by hand from X.680 §47.3 Example 2 |
| UT07d | same | | DER | ERR DERFORM | X.690 §11.8.2 |
| UT08 | `920622123421+0100` | `17 11 3932303632323132333432312B30313030` | BER | valid; DER ERR DERFORM | by hand from X.680 §47.3 c) 2) |
| UT09 | `920622123421` | `17 0C 393230363232313233343231` | both | ERR TSYNTAX (no Z and no offset; §47.3 c) mandatory) | by hand from X.680 §47.3 c) |
| UT10 | `920622123461Z` | `17 0D 3932303632323132333436315A` | both | ERR TRANGE (ss=61 > 59; §47.3 b) 2)) | by hand from X.680 §47.3 b) 2) |
| UT11 | `921322123421Z` | `17 0D 3932313332323132333432315A` | both | ERR TRANGE (month 13: §47.3 says only "counting January as 01"; rejection derives from "calendar date", §47.2 a)) | by hand |
| UT12 | `491231235959Z` | `17 0D 3439313233313233353935395A` | both | valid; YY=49. Standard: no century rule (section 3.4). Under RFC 5280 profile (outside the ITU texts): 2049 | by hand from X.680 §47.3 a) |
| UT13 | `500101000000Z` | `17 0D 3530303130313030303030305A` | both | valid; YY=50. Standard: no century rule. RFC 5280 profile: 1950 | by hand from X.680 §47.3 a) |
| UT14 | `000101000000Z` | `17 0D 3030303130313030303030305A` | both | valid; YY=00; profile: 2000 | by hand from X.680 §47.3 a) |
| UT15 | `991231235959Z` | `17 0D 3939313233313233353935395A` | both | valid; YY=99; profile: 1999 | by hand from X.680 §47.3 a) |
| UT16 | `920622123421Z` as two segments "92062" + "2123421Z" | `37 11 04 05 39 32 30 36 32 04 08 32 31 32 33 34 32 31 5A` | BER | valid (DERIVED, conflict C4) | by hand from X.690 §8.25 -> §8.23.3 -> §8.7.3 |
| UT16d | same | same | DER | ERR DERFORM (DERIVED via §10.2) | by hand |

| ID | string | full hex | mode | expected | provenance |
|---|---|---|---|---|---|
| GT01 | `19920521000000Z` | `18 0F 31393932303532313030303030305A` | both | valid | by hand from X.690 §11.7.5 (example string) |
| GT02 | `19920622123421Z` | `18 0F 31393932303632323132333432315A` | both | valid | by hand from X.690 §11.7.5 |
| GT03 | `19920722132100.3Z` | `18 11 31393932303732323133323130302E335A` | both | valid, fraction .3 | by hand from X.690 §11.7.5 |
| GT04 | `19920622123421.0Z` | `18 11 31393932303632323132333432312E305A` | BER valid; DER | ERR DERFORM (spurious trailing zero, §11.7.3; invalid example in §11.7.5) | by hand from X.690 §11.7.5 |
| GT05 | `19920722132100.30Z` | `18 12 31393932303732323133323130302E33305A` | BER valid; DER | ERR DERFORM | by hand from X.690 §11.7.5 |
| GT06 | `19920520240000Z` | `18 0F 31393932303532303234303030305A` | both | ERR TRANGE (hh=24; X.690 §11.7.5; X.680 §46.2 excludes end-of-day midnight) | by hand from X.690 §11.7.5 |
| GT07 | `19851106210627.3` (local, no zone) | `18 10 31393835313130363231303632372E33` | BER | valid (case a) | from X.680 §46.3 example case a), framed by hand |
| GT07d | same | | DER | ERR DERFORM (Z missing, §11.7.1) | |
| GT08 | `19851106210627.3Z` | `18 11 31393835313130363231303632372E335A` | both | valid (case b) | from X.680 §46.3 example case b), framed by hand |
| GT09 | `19851106210627.3-0500` | `18 15 31393835313130363231303632372E332D30353030` | BER | valid; UTC 1985-11-07T02:06:27.3 (case c); DER ERR DERFORM | from X.680 §46.3 example case c), framed by hand |
| GT10 | `198511062106.456` | `18 10 3139383531313036323130362E343536` | BER | valid: fraction of minute (case d); DER ERR DERFORM (seconds absent, §11.7.2) | from X.680 §46.3 case d), framed by hand |
| GT11 | `1985110621.14159` | `18 10 313938353131303632312E3134313539` | BER | valid: fraction of hour (case e); DER ERR DERFORM | from X.680 §46.3 case e), framed by hand |
| GT12 | `19851106210627,3Z` | `18 11 31393835313130363231303632372C335A` | BER | valid (comma allowed, §46.3 a) 2)); DER ERR DERFORM (point only, §11.7.4) | by hand from X.680 §46.3 a) 2) and X.690 §11.7.4 |
| GT13 | `19851106210627.500Z` | `18 13 31393835313130363231303632372E3530305A` | BER valid; DER ERR DERFORM (trailing zeros) | by hand from X.690 §11.7.3 |
| GT14 | `19851106210627.5Z` | `18 11 31393835313130363231303632372E355A` | both | valid | by hand from X.690 §11.7.3 |
| GT15 | `1985110621Z` | `18 0B 313938353131303632315A` | BER | valid (hour precision); DER ERR DERFORM | by hand from X.680 §46.2 a) 3) |
| GT16 | `19851106210627.3+05` | `18 13 31393835313130363231303632372E332B3035` | BER | valid (minutes of offset omitted, §46.3 c)); DER ERR DERFORM | by hand from X.680 §46.3 c) |
| GT17 | `19851106210627+0000` | `18 13 31393835313130363231303632372B30303030` | BER | valid (offset zero is not the same encoding as Z); DER ERR DERFORM | by hand from X.690 §11.7.1 |
| GT18 | `19851106215960Z` | `18 0F 31393835313130363231353936305A` | both | POLICY: seconds 60 (leap second) - no range stated for GeneralizedTime seconds in X.680 §46; text silent | by hand |
| GT19 | `20240229000000Z` | `18 0F 32303234303232393030303030305A` | both | valid (leap day) | by hand from X.680 §46.2 "calendar date" |
| GT20 | `19851106210627.Z` | `18 10 31393835313130363231303632372E5A` | both | POLICY ERR TSYNTAX (decimal sign with no digit: grammar says a fraction, no example without digits; ISO 8601 not available) | by hand |

### 5.7 TIME, DATE, TIME-OF-DAY, DATE-TIME, DURATION

| ID | hex input | mode | expected | provenance |
|---|---|---|---|---|
| TM01 | `1F 1F 08 3230323130323135` | both | DATE 2021-02-15 | by hand from X.690 §8.26.2.2: value notation "2021-02-15" minus quotes and hyphens = 20210215; tag 31 = 1F 1F per §8.1.2.4 |
| TM02 | `1F 20 06 313233303435` | both | TIME-OF-DAY 12:30:45 | by hand from X.690 §8.26.3.2: "12:30:45" minus colons |
| TM03 | `1F 21 0E 3230323130323135313233303435` | both | DATE-TIME 2021-02-15T12:30:45 | by hand from X.690 §8.26.4.2: remove hyphens, colons, T |
| TM04 | `1F 22 0D 3159324D3344543448354D3653` | both | DURATION P1Y2M3DT4H5M6S | by hand from X.690 §8.26.5.2: remove P |
| TM05 | `0E 0A 323032312D30322D3135` | both | TIME "2021-02-15" (Basic=Date YMD) | by hand from X.690 §8.26.1.2 (only quotes removed; hyphens stay) |
| TM06 | `0E 14 323032312D30322D31355431323A33303A34355A` | both | TIME "2021-02-15T12:30:45Z" | by hand from X.690 §8.26.1.2 and X.680 Table 7 |
| TM07 | `0E 19 323032312D30322D31355431323A33303A34352B30353A3330` | both | TIME "...+05:30" (non-integral hour difference keeps minutes, X.680 Table 7) | by hand from X.690 §8.26.1.2 |
| TM08 | `0E 19 ...+05:00` (`323032312D30322D31355431323A33303A34352B30353A3030`) | BER valid; DER | ERR DERFORM (must be `+05`, §11.9.1 b) | by hand from X.690 §11.9.1 b) |
| TM09 | `0E 16 323032312D30322D31355431323A33303A34352B3035` | both | valid ("+05") | by hand from X.690 §11.9.1 b) |
| TM10 | TIME "2021-02-15T12:30:45,5Z" = `0E 16 323032312D30322D31355431323A33303A34352C355A` | BER valid; DER | ERR DERFORM (comma, §11.9.1 a) | by hand from X.690 §11.9.1 a) |
| TM11 | `1F 22 05 32394D3044` ("29M0D") | both | valid canonical DURATION P29M0D | by hand from X.690 §11.9.1 d) on X.680 §38.3.6.14 example |
| TM12 | `1F 22 07 30593239 4D3044` (contents `0Y29M0D`) | BER valid; DER | ERR DERFORM (redundant zero component, §11.9.1 d) | by hand from X.690 §11.9.1 d) |
| TM13 | `1F 22 08 32394D54302E3553` ("29MT0.5S") | both | valid | by hand from X.680 §38.3.6.12 |
| TM14 | `3F 1F 08 3230323130323135` | any | ERR FORM (constructed DATE; X.690 §8.26.2.1) | by hand: identifier 1F 1F with bit 6 set = 3F 1F, otherwise as TM01 |

---------------------------------------------------------------------------------------------------------------------

## 6. Statements NOT found / not verified

1. A century rule for the two-digit UTCTime year in X.680 or X.690: not found (section 3.4). The 50 pivot exists only in RFC 5280 §4.1.2.5.1.
2. What a receiver SHALL do with an octet outside a string alphabet, a malformed UTF-8/UCS-2/UCS-4 sequence, an out-of-range time field, or an odd-length BMPString: not found. X.690 states only encoder rules plus §7.3 / §8.23.6 ¶2 (receivers accept all permitted alternatives). All reject/accept actions beyond that are DERIVED or POLICY as marked.
3. Whether UTF8String forbids surrogates (U+D800-DFFF), values above U+10FFFF, and what "truncated" means beyond length: not found in X.680/X.690; decided by ISO/IEC 10646 Annex D (X.690 §8.23.10 ¶1), which is not in the corpus. Not verified.
4. UniversalString: any limit (e.g. above U+10FFFF, surrogates): not found; X.680 §41.6 says "any of the characters allowed by ISO/IEC 10646"; not verified.
5. The identity of the two cells excluded by "64K-2" in BMPString (X.680 §41.15) beyond the surrogate area: not found; inferred U+FFFE and U+FFFF only.
6. Code-point tables for TeletexString, VideotexString, GraphicString, GeneralString and the contents of ISO register entries 1, 6, 87, 102 etc.: not in the corpus; ISO 2022/2375 not read. Section 2.5.
7. The ISO 8601 definitions the time grammars cite (4.1.2.2, 4.2.2.2-4, 4.2.5.2): not available; therefore not found: valid ranges for GeneralizedTime seconds (leap second 60), the range of offset hours/minutes, whether a decimal sign without digits is allowed, and calendar validity rules (days per month) beyond "calendar date".
8. Any statement that GeneralizedTime/UTCTime may be constructed in BER or not in DER: not found directly; derived (conflict C4).
9. DER/CER canonical forms for general TIME values (intervals, recurrence, week and ordinal dates, centuries, large years): X.690 §11.9.1 a)-d) was read; X.680 Table 6/7 only partially transcribed here (only what DATE, TIME-OF-DAY, DATE-TIME, DURATION need). A complete TIME parser needs a full read of X.680 §38.
10. X.680 2008 edition text for §41, §46, §47 (comparison requested): not in the corpus; cannot compare. X.690 2002 edition and Amendment 2 (06/2006): text not in the corpus.
11. Tables were read from the text layer only; no visual rendering of the PDF pages was possible. Alphabet tables were cross-checked by two extraction modes, UTF-8 byte dump (PrintableString) and the in-document counts.
12. Erratum 1 (09/2021) affects only X.690 Figure 4; I did not need its content for this slice.
13. All hex test vectors were produced by hand with `od`/`printf`; none was run through a decoder.