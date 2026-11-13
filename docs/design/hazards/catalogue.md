# Hazard catalogue: asn1core checked against X.690 / X.680 (02/2021)

Method: every clause was read with `itu_read` on `X.690#2021-02` / `X.680#2021-02`; citations are
`X.690 (02/2021) §clause ¶n` (¶ = sentence number in the itulib extraction). Behaviour claims about
asn1core were measured by running `decode()` read-only (`python -B`, nothing written to the repo) on
hand-built bytes; those are marked **[probed]**. asn1core is a list of hazards here, not a reference.
Citations below drop the "X.690 (02/2021)" prefix when the clause is in X.690; X.680 clauses say so.

Verdict key: **AGREES** (behaviour matches the clause) / **DEVIATES** (differs from the clause) /
**POLICY** (standard does not decide; asn1core made a choice) / **OPEN** (the standard text available
to me does not settle it) / **NOT COVERED** (asn1core does not implement it).
Rust column: E = encode, B = decode as BER, D = decode as DER.

## A. Identifier octets

| # | Hazard | What goes wrong | Governing clause | Rust crate must | asn1core |
|---|---|---|---|---|---|
| H01 | Tag number >= 31 (high-tag-number form) | accumulating `(n<<7)\|payload` wrongly: snmpy gave 3999 for 31 (claimed in PLAN, snmpy not re-run by me); encode and decode disagree | §8.1.2.4.1 ¶1 (first octet bits 5-1 all ones); §8.1.2.4.2 ¶1 a,b (base-128, bit 8 continuation, MSB group first) | E: emit minimal groups. B/D: accumulate once per octet, check overflow of your integer type | AGREES (tag.py; tests pin 31, 63, 127, 128, 16383, 16384, 9999) |
| H02 | Class bits and constructed bit isolated, not OR-ed | `0x5F` vs `0x7F`, `0xC1` vs `0x81`; for high tags bit 6 is the only place constructedness lives | §8.1.2.2 ¶1-2 + Table 1; §8.1.2.4.1 ¶1 a,b; §8.1.2.5 ¶1 | model (class, constructed, number) as three fields; never OR | AGREES |
| H03 | Tag number 0-30 must use the single-octet form | `9F 05 00` is tag 5 spelled long | §8.1.2.2 ¶1 (tags 0 to 30 comprise a single octet) | B: reject (invalid BER). D: reject | DEVIATES in BER **[probed]**: BER accepts `9F 05 00`; DER refuses (rule `tag-number-not-minimal`). VECTORS says "BER permits it"; no clause says so |
| H04 | Leading 0x80 group in a high tag number | non-minimal tag number | §8.1.2.4.2 ¶1 c (bits 7-1 of the first subsequent octet shall not all be zero). Clause 8 is BER, with no DER-only qualifier | B: reject. D: reject. Test "first subsequent octet == 0x80", not "value < 31" | DEVIATES twice **[probed]**: (1) BER accepts `9F 80 01 00`; (2) the DER check is `number < 31`, so `9F 80 1F 00` (tag 31) and `9F 80 81 00 00` (tag 128) pass DER. A real hole in the strict path |
| H05 | Unbounded tag number | hostile continuation octets build an unbounded integer | standard sets no ceiling (silent) | cap (u32/u64), refuse above it with a named error, document it | POLICY (2**32 ceiling) |
| H06 | Universal tag numbers above 30 exist | DATE, TIME-OF-DAY, DATE-TIME, DURATION are universal 31-34, OID-IRI 35, relative OID-IRI 36; a decoder assuming universal <= 30 breaks | X.680 §8.6 ¶2 (Table 1) | tag type carries numbers >= 31 in every class including universal | AGREES (generic `Tag`; `1F 1F 00` parses **[probed]**) |
| H07 | Universal tag 0 is reserved for the encoding rules; 15 reserved | `00 01 AA` read as an ordinary element | X.680 §8.6 ¶2 (Table 1: UNIVERSAL 0 reserved for the encoding rules) | refuse universal 0 as an element; EOC recognised only by the indefinite-length reader | DEVIATES **[probed]**: `00 01 AA` and `30 80 00 01 AA 00 00` parse as tag-0 nodes |

## B. Length octets and framing

| # | Hazard | What goes wrong | Governing clause | Rust crate must | asn1core |
|---|---|---|---|---|---|
| H10 | Length encode at exact powers of 256 | float-log sizing gave `OverflowError` at 256, 65536, 2**24 | §8.1.3.5 ¶4 (big-endian unsigned integer in the subsequent octets) | E: octets = ceil(bits/8) via integer ops, never floats | AGREES (`bit_length`; tests at every 256**k) |
| H11 | Non-minimal long-form length | `04 81 7F`, `04 82 00 7F` | BER legal: §8.1.3.5 ¶5 NOTE 2 (sender's option to use more length octets). DER: §10.1 ¶1 (minimum number of octets) | B: accept. D: reject. E: minimal always | AGREES (but see H13 cap) |
| H12 | Reserved length byte 0xFF | read as 127 length octets | §8.1.3.5 ¶2 c (11111111 shall not be used) | B/D: reject | AGREES |
| H13 | Long form with a large octet count (`8F`, `89`) | wrong rationale and an arbitrary cap | only 0xFF is reserved (§8.1.3.5 ¶2 c). Counts up to 126 are legal; leading-zero padding is legal BER (¶5 NOTE 2) | B: accept any count whose numeric *value* fits your limit (skip leading zeros). Never describe 0x8F as reserved | DEVIATES (rationale) + POLICY: VECTORS row `len-undefined-high` and its test docstring say "low nibble 15 is reserved by X.690": false. `04 89 00*8 01 41` is refused by the 8-octet cap **[probed]** though legal BER |
| H14 | Length from the wire used as allocation size | `04 88 FF*8` = 1.8e19 bytes | standard silent (lengths are octet counts, §8.1.3.3 ¶1) | compare with the remaining slice before use; `checked_add` for `start+len` (release builds wrap) | POLICY (64 MiB absolute cap plus buffer check). A fixed cap also rejects legal large BER; make it a caller parameter |
| H15 | Truncated input at any depth | `IndexError`, silent short value, offset past end | standard silent on error handling; framing §8.1.3.3 ¶1 | `Err(Truncated{offset})`, never panic | AGREES in effect (TruncatedError everywhere) |
| H16 | Length 0 versus absent | "empty" confused with "no element" | §8.1.4 ¶1 (contents zero, one or more octets); §8.1.3.4 ¶1 | distinct representations | AGREES |
| H17 | Children must tile the parent | inner length overruns parent yet outer cursor "succeeds" | §8.9.2 ¶1 (contents = complete encodings in sequence); §8.6.3 ¶1, §8.7.3 ¶1 | parse children inside the parent's slice | AGREES (`limit` plus a tile check **[probed]**: `30 03 30 80 00 00` refused) |
| H18 | Indefinite length: terminator searched in content | `buffer.index(b"\0\0")` finds data zeros | §8.1.3.6 ¶1 (contents terminated by EOC); §8.1.5 ¶1-2 (EOC = two zero octets, "can be considered" a tag-0 TLV, ¶3) | B: parse element by element; EOC only at an element boundary; indefinite nests | AGREES **[probed]** (`24 80 04 02 00 00 00 00` ok; nested indefinite ok) |
| H19 | Indefinite length on a primitive encoding | meaning of "content" undefined | §8.1.3.2 ¶1 a (sender shall use the definite form if primitive); §7.3 ¶2 (receivers support the alternatives that are sender options) | B/D: reject | AGREES (inferred; **[probed]** `04 80 41 00 00` refused) |
| H20 | Indefinite length in DER | | §10.1 ¶1 | D: reject | AGREES |
| H21 | Stray EOC | `00 00` top-level or inside a definite constructed accepted | §8.1.5 ¶1 (EOC present only if the length is indefinite); §8.1.1.2 ¶1 | refuse EOC unless it closes an indefinite constructed | DEVIATES **[probed]**: `00 00` and `30 02 00 00` accepted by BER and DER |
| H22 | EOC beyond the parent's extent | indefinite child's EOC read past a definite parent | §8.1.3.3, §8.1.5 | check against the parent limit | AGREES in effect (caught by the tile check; message generic) |
| H23 | Nesting depth | stack overflow | standard silent | explicit depth limit | POLICY (MAX_DEPTH 64; named error, **[probed]** 70 levels refused, 60 ok) |
| H24 | Trailing octets after the top-level object | "ok" with junk | standard silent (a data value is one TLV, §8.1.1.1) | API returns consumed length; strict helper refuses trailing | POLICY (`decode` returns end; `validate_der` refuses) |

## C. Primitive vs constructed form (bit 6) by type

| # | Hazard | What goes wrong | Governing clause | Rust crate must | asn1core |
|---|---|---|---|---|---|
| H30 | Form is fixed by type: primitive BOOLEAN, INTEGER, ENUMERATED, REAL, NULL, OID; constructed SEQUENCE, SEQUENCE OF, SET, SET OF | `10 00` (primitive SEQUENCE), `22 03 02 01 00` (constructed INTEGER) | §8.2.1 ¶1, §8.3.1 ¶1, §8.4 ¶2 (NOTE), §8.5.1 ¶1, §8.8.1, §8.9.1 ¶1, §8.10.1 ¶1, §8.11.1 ¶1, §8.12.1 ¶1, §8.19.1 ¶1 | B and D: reject the wrong form for universal types | DEVIATES **[probed]**: BER accepts primitive SEQUENCE/SET and constructed INTEGER/BOOLEAN; DER refuses constructed INTEGER/BOOLEAN only by accident (empty-content check) and accepts primitive `10 00` and `11 00` |
| H31 | BIT STRING / OCTET STRING constructed (BER) | no reassembly; segments with unused bits; nesting | §8.6.1 ¶1; §8.6.3 ¶1; §8.6.4 ¶2 (non-final segments are whole octets); §8.6.4.1 ¶1-3,6 (segments in order, tag universal 3); §8.7.1, §8.7.3; §8.23.6 ¶2; §7.3 ¶2 (receiver shall support) | B: reassemble (flatten recursively), expose a "value" accessor | NOT COVERED: the tree parses, but `Node.content()` of a constructed string is the concatenated child TLVs, not the string value |
| H32 | Constructed string forms in DER | one value, two encodings | §10.2 ¶1 (bitstring, octetstring, restricted character string: constructed form shall not be used) | D: reject constructed for those universal tags | AGREES for tags 3, 4, 12, 18-22, 25-28, 30 |
| H33 | Constructed UTCTime / GeneralizedTime / ObjectDescriptor under DER | defined as IMPLICIT VisibleString / GraphicString, so BER can construct them (§8.25 ¶1), but §10.2 does not name them | §8.25 ¶1; X.680 §47.3, §46.3, §48.3; §10.2 ¶1 | decide explicitly; reject in D | OPEN (the text does not list them). asn1core refuses 23/24 only by accident (the content check sees empty content, by reading the code, not probed) and excludes 7 on purpose |

## D. Values

| # | Hazard | What goes wrong | Governing clause | Rust crate must | asn1core |
|---|---|---|---|---|---|
| H40 | Non-minimal INTEGER | `02 02 00 01` | §8.3.2 ¶1 (first octet and bit 8 of the second shall not be all ones, nor all zeros); ¶2 NOTE says this ensures the smallest form. Clause 8 is BER | B: reject (not valid BER). D: reject. Offer an explicit lenient flag if you must read bad data | DEVIATES in BER **[probed]**: accepted; VECTORS/README call it "legal BER". DER agrees |
| H41 | Empty INTEGER content | `02 00` | §8.3.1 ¶2 (one or more octets) | B/D: reject (also ENUMERATED) | DEVIATES in BER **[probed]**: accepted; VECTORS `int-zero-empty` says reject both; DER rejects |
| H42 | INTEGER sign: -128, 128, -129; padded ECDSA r | stripping the load-bearing 00 breaks signatures; treating 80 as overflow | §8.3.3 ¶1,4 (two's complement, bit 8 of first octet negative) | keep the raw content slice; sign from first octet; never strip zeros | AGREES (`02 01 80` = -128; `FF 80` refused as redundant) |
| H43 | ENUMERATED | same rules as INTEGER, unchecked | §8.4 ¶1 | apply H40/H41 | DEVIATES **[probed]**: `0A 02 00 01` accepted in DER |
| H44 | BOOLEAN | `01 01 02`; wrong length | §8.2.1 ¶2 (single octet); §8.2.2 ¶2-3 (FALSE zero, TRUE any non-zero, sender's option); §11.1 ¶1 (DER: all eight bits one) | B: length is 1, nonzero is true. D: only 00 / FF | AGREES in DER; DEVIATES in BER **[probed]**: `01 02 FF FF` and `01 00` accepted |
| H45 | NULL | `05 01 00` | §8.8.2 ¶1 (no contents octets) | B/D: reject | DEVIATES **[probed]**: accepted by BER and DER |
| H46 | BIT STRING initial octet | count > 7; `03 00`; nonzero count with no data | §8.6.2.2 ¶1-2 (0..7); §8.6.2.3 ¶1 (empty value: no subsequent octets, initial octet zero); §8.6.2 ¶1 | B/D: reject zero octets, count > 7, nonzero count with no data | AGREES in DER; DEVIATES in BER **[probed]**: `03 00`, `03 02 09 00`, `03 01 03` accepted |
| H47 | Unused bits must be zero | | §11.2.1 (DER/CER; BER leaves them free) | D only | AGREES |
| H48 | Named bit lists: trailing zero bits | keyUsage encodings | §11.2.2 ¶1,3 (only where X.680 §22.7 applies; no 1 bits gives `03 01 00`); §8.6.2.4 ¶1-2 (BER may add or remove trailing 0 bits); X.680 §22.7 ¶1 | typed layer, not the generic decoder | AGREES (typed decoders; corrected the earlier "empty is illegal" mistake) |
| H49 | OID first two arcs packed | refusing 80-127 (`06 02 7F 01`) kills every `2.5.x` | §8.19.4 ¶1-3 (X*40+Y; NOTE: three root values, at most 39 under 0 and 1) | decode the first subidentifier as a full base-128 value, then split at 40/80. E: `40*X+Y` as base-128 (can exceed 127: `2.999.3` = `88 37 03`, §8.19.5 ¶2-4) | AGREES **[probed]** (`88 37 03` = 2.999.3; `55 1D 13` = 2.5.29.19) |
| H50 | OID subidentifier continuation | encoder and decoder agreeing wrongly (`2.100.3` as `B4 03`) | §8.19.2 ¶2-5 (bit 8 is the continuation flag) | test against an independent decoder | AGREES |
| H51 | Non-minimal OID subidentifier | `06 02 80 01` | §8.19.2 ¶6 (leading octet of a subidentifier shall not be 0x80). Clause 8 is BER | B/D: reject | DEVIATES in BER **[probed]**: accepted (`decode_oid` is tolerant and node decoding never calls it); DER agrees |
| H52 | OID arcs above 32 bits | `2.25.<128-bit UUID>` | §8.19.2 (no limit on subidentifier size) | arbitrary-size arcs (bignum or byte-slice view); cap only as a stated policy | DEVIATES/POLICY **[probed]**: arcs above 2**32 refused on decode and encode; 64-arc cap |
| H53 | Empty OID content; OID needs two arcs | | X.680 §32.11 ¶2 NOTE (at least two arcs, per X.660); §8.19.3 ¶1 | reject empty | AGREES (inferred) |
| H54 | Invalid dotted OIDs repaired silently (`3.0` becomes `2.40` under `python -O`) | | the first-arc / second-arc limits are X.660 rules, not in the X.680/X.690 text I could read (§8.19.4 NOTE only) | real errors, checked arithmetic; no `debug_assert!` on wire data | OPEN for the exact arc rule (X.660 not read); implementation hazard handled |
| H55 | UTCTime forms | BER allows hhmm only, optional seconds, `+hhmm` | X.680 §47.3 ¶2 b, c (hhmm or hhmmss; Z or +/-hhmm); DER: §11.8.1, §11.8.2 | B: parse all forms or keep raw; D: `YYMMDDHHMMSSZ` only | AGREES (BER keeps octets; DER 13 octets ending Z) |
| H56 | UTCTime century | YY has no century in the standard | X.680 §47.3 ¶2 a (YY = two low-order digits of the year) | century window is a profile parameter (RFC 5280: >= 50 means 19YY) | POLICY (correctly attributed to RFC 5280) |
| H57 | Seconds = 60 | | UTCTime: X.680 §47.3 ¶2 b 2 (ss 00 to 59). GeneralizedTime: §46.3 defers to ISO 8601 (not in the corpus) | UTCTime: reject 60. GeneralizedTime: decide | AGREES for UTCTime. OPEN for GeneralizedTime (asn1core refuses 60 there too). Stale text in `time.py` docstring and in `test_a_leap_second_is_refused...` says X.690 "permits 60" and keeps a marker: contradicted by §47.3 |
| H58 | GeneralizedTime in DER | fractional seconds | §11.7.1 (Z), §11.7.2 (seconds present), §11.7.3 ¶1 (fraction allowed, no trailing zeros, omit the point if zero), §11.7.4; example valid `19920722132100.3Z` (§11.7.5 ¶2) | D: accept `YYYYMMDDHHMMSS[.f+]Z` with the no-trailing-zero rule; a PKIX profile layer then forbids fractions | DEVIATES from X.690 DER **[probed]**: `19920722132100.3Z` refused as `time-not-canonical`. It is the RFC 5280 profile labelled "DER" |
| H59 | GeneralizedTime BER forms | local time, `+hhmm`, hour-only, minute fractions, comma decimal | X.680 §46.3 ¶2-5, examples ¶14-18 | B: parse or keep raw | NOT COVERED (raw octets only) |
| H60 | Midnight | `...240000Z` | §11.7.5 ¶1, §11.8.3 (midnight is next day 000000) | D: reject hour 24 | AGREES **[probed]** |
| H61 | Calendar validity (30 Feb) | | X.680 §47.3 ¶2 a states only DD 01-31; calendar validity is implicit in "calendar date" | reject impossible dates | POLICY / inference (via `datetime`) |
| H62 | Year range; 2049 boundary | year 0000; the 2050 rule | the 2049/2050 rule is RFC 5280 §4.1.2.5; X.680/X.690 silent | integer repr spanning 0000-9999; keep the 2049 rule in the profile | POLICY (reports, does not enforce); Python `datetime` cannot hold year 0000 (implementation limit) |
| H63 | PrintableString alphabet | `@ & _ é` inside | X.680 §41.4 ¶2-5 (Table 10: 26+26+10, SPACE, 11 specials = 74) | D: reject; B: accept and report | AGREES (set equals Table 10) / POLICY (enforced only on the DER path; lenient mode records a `Defect`) |
| H64 | NumericString and other alphabets | | X.680 §41.2 ¶2 (Table 9: digits and SPACE) | typed accessor decides | POLICY (not enforced); IA5 / Visible not checked either |
| H65 | Character string type numbers | tags 13/14 mistaken for strings | X.680 §8.6 ¶2 (Table 1: 13 relative OID, 14 TIME, 18-22, 25-30); §41 ¶4 (Table 8: 12, 18, 19, 20, 21, 22, 25, 26, 27, 28, 30) | one table, tested against the standard | AGREES (fixed; tag 29 CHARACTER STRING declined) |
| H66 | BMPString / UniversalString / UTF8String content | surrogates, BOM, overlong | §8.23.8 ¶1-2 (2-octet BMP form, no signature); X.680 §41.15 ¶1 (BMP only; surrogate cells excluded); §8.23.7 ¶1-2 (4-octet form); §8.23.10 ¶1-2 (UTF-8, shortest form) | validate in the typed layer; BMP: reject surrogate code units | DEVIATES for BMP: `utf-16-be` accepts valid surrogate pairs (reading of code, not probed). UTF-8 strictness AGREES (Python strict decoder) |
| H67 | 8-bit string types read as Latin-1 | | §8.23.5 ¶1 (ISO/IEC 2022 8-bit environment with escape sequences) | decide per type: Printable/IA5/Numeric/Visible are 7-bit; T61/General/Graphic/Videotex need ISO 2022 | POLICY (Latin-1; UTF-8-then-Latin-1 fallback for Teletex/Videotex/Graphic) |
| H68 | REAL | | §8.5.2, §8.5.6 ¶1, §8.5.7, §8.5.9, §11.3 | implement or refuse by name | NOT COVERED |

## E. Canonical ordering and DEFAULTs

| # | Hazard | What goes wrong | Governing clause | Rust crate must | asn1core |
|---|---|---|---|---|---|
| H70 | SET OF order | unsorted RDN sets compare unequal | §11.6 ¶1-2 (ascending, compared as octet strings, shorter zero-padded for comparison only) | D: check ascending; E: sort the encodings. Plain lexicographic compare of complete DER TLVs is equivalent (no DER TLV is a proper prefix of another) | AGREES |
| H71 | SET (not SET OF) order | applying SET OF's byte order to every SET | §10.3 ¶1 (order by tag per X.680 §8.6 ¶1: universal, application, context, private; ascending number inside a class); not by encoding | D: a schema-less decoder cannot tell SET from SET OF; expose both checks or defer to the typed layer. Byte order is wrong for SET because the constructed bit (bit 6) outranks the number bits | DEVIATES **[probed]**: every tag-17 node gets the SET OF check. `31 04 A0 00 81 00` (components [0], [1]: tag order, valid DER) is refused; `31 04 81 00 A0 00` (wrong tag order) is accepted |
| H72 | DEFAULT components omitted in DER | `critical FALSE` written explicitly | §11.5 ¶1 | typed layer; E never emits defaults; D rejects an explicit default | AGREES in requirement; asn1core's own ROADMAP row `dk.critical-default` is `[ ]` (explicit `01 01 00` still accepted under DER) |
| H73 | EXPLICIT vs IMPLICIT tags | wrong constructed bit; implicit read as explicit | §8.14.3 ¶1 (explicit: constructed, contents the full base encoding); §8.14.4 ¶1 (implicit: form follows the base encoding); X.680 §31.2.9 (no IMPLICIT on an untagged CHOICE) | typed layer decides; expose class, number, constructed; check form against the schema | AGREES in design (`is_context(n)` ignores the bit on purpose) |

## F. Design and security hazards from the project

| # | Hazard | Governing basis | Rust crate must | asn1core |
|---|---|---|---|---|
| H80 | Signature verified over re-encoded bytes | §7.3 ¶1 (BER allows alternative encodings, so re-encoding need not reproduce the input); RFC 5280 §4.1.1.3 is the actual requirement; §7.4 ¶1 (DER has no alternatives) | every node keeps the exact input slice (`&'a [u8]`, zero copy) | AGREES (`Node.raw`; 67/67 self-signed store certs verify per PLAN, not re-run by me) |
| H81 | BER leniency silently turned into canonical form | §7.4 ¶1 | decode never normalises; strict and lenient modes with named violations | POLICY (design) |
| H82 | Lenient values, defects as data | standard silent (an out-of-alphabet string is not in the type's value set, X.680 §41.4) | return `(value, Vec<Defect{rule, offset}>)`; keep DER spelling rules enforced | POLICY |
| H83 | Collect-all versus first violation | none | state which; collect-all is better for triage | POLICY (`der_violations` lists the first spelling fault, then value faults) |
| H84 | `assert` stripped under `-O` | Python only | Rust analogue: no `debug_assert!`, `unwrap`, or unchecked arithmetic on wire data | n/a (handled; AST test) |
| H85 | Test-construction traps | §8.3.3 ¶1,4 (`80 00` is -32768, no negative zero); §8.1.2.4.2 (`9F 81 04` is tag 132, not `04`) | mutation tests must recompute enclosing lengths; derive expectations from the clause | AGREES (recorded in PLAN dk.corpus) |
| H86 | Fields located by tag, not index | §11.5 ¶1 (DEFAULT omitted, so v1 shifts fields) | typed layer walks by tag | AGREES |
| H87 | Signature BIT STRING unused bits | §8.6.2.2 ¶1 plus profile | strip the initial octet, require 0 | AGREES |
