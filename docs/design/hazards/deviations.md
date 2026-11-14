# Where asn1core agrees, deviates, or chooses (X.690 / X.680, 02/2021)

Tally of `catalogue.md` (67 hazards; a row mixing two verdicts is counted by its stronger one):
AGREES 31, DEVIATES 17, POLICY 12, OPEN 3, NOT COVERED 3, not applicable (Python `-O`) 1.
Ids refer to catalogue rows. **[probed]** = I ran `decode()` read-only on hand-built bytes.

## 1. Deviations from the standard (ranked by consequence for a Rust crate)

1. **Non-minimal tag number accepted in DER when the number is >= 31 (H04).** DER's check is
   `number < 31`, so `9F 80 1F 00` and `9F 80 81 00 00` pass `der=True` **[probed]**.
   X.690 (02/2021) §8.1.2.4.2 ¶1 c: bits 7-1 of the first subsequent octet shall not all be zero.
   Also, BER accepts the same bytes, although that clause is in clause 8 (BER) and has no DER qualifier.
2. **The strictness line is drawn in the wrong place: several "BER tolerates" claims are not BER
   (H03, H04, H40, H41, H51).** VECTORS/README say BER accepts `02 02 00 01`, `9F 80 01 00`,
   `9F 05 00` and `06 02 80 01`. The shall-clauses are all in clause 8: §8.3.2 ¶1 (minimal INTEGER),
   §8.3.1 ¶2 (non-empty INTEGER), §8.1.2.2 ¶1 and §8.1.2.4.2 ¶1 c (tag numbers), §8.19.2 ¶6
   (subidentifier, leading 0x80). The only encoding freedoms a BER receiver must accept are the sender
   options (§7.3 ¶1-2): long-form length padding (§8.1.3.5 ¶5 NOTE 2), BOOLEAN TRUE as any non-zero
   (§8.2.2 ¶3), constructed strings (§8.6.1, §8.7.1), definite-or-indefinite constructed (§8.1.3.2 b).
   So these are policy leniency (to read bad certificates), not standard behaviour. A Rust crate should
   make BER = "all sender options, nothing else", and offer an explicit named lenient mode for the rest.
3. **BER decode checks no value rules at all (H30, H41, H43-H46).** Framing only. `02 00`, `01 02 FF FF`,
   `05 01 00`, `03 00`, `03 02 09 00`, constructed INTEGER, primitive SEQUENCE are accepted **[probed]**.
   Clauses: §8.3.1 ¶2, §8.2.1 ¶2, §8.8.2 ¶1, §8.6.2.2 ¶2, §8.6.2.3 ¶1, §8.3.1 ¶1, §8.9.1 ¶1.
   VECTORS's own rows `int-zero-empty` and `bits-empty` say "reject in both BER and DER"; the code does not.
4. **A SET gets SET OF's ordering rule (H71).** `check_set_of` runs on every tag-17 node. DER orders SET
   components by tag (X.690 §10.3 ¶1, X.680 §8.6 ¶1), not by encoding (§11.6 is SET OF only). **[probed]**:
   a valid DER SET `31 04 A0 00 81 00` is refused and the wrong order `31 04 81 00 A0 00` is accepted,
   because the constructed bit sorts above the number bits.
5. **GeneralizedTime "DER" is the RFC 5280 profile (H58).** §11.7.3 ¶1 allows a fractional part with no
   trailing zeros; the standard's own valid example `19920722132100.3Z` (§11.7.5 ¶2) is refused **[probed]**.
   Correct as a PKIX profile; wrong as X.690 DER. A Rust crate needs two layers.
6. **Stray EOC and universal tag 0 accepted as elements (H07, H21).** `00 00`, `30 02 00 00`, `00 01 AA`
   parse **[probed]**. §8.1.5 ¶1: EOC present only when the length is indefinite; X.680 Table 1 reserves
   UNIVERSAL 0 for the encoding rules.
7. **Form bit not checked against the type (H30).** §8.9.1 ¶1 etc. (see 3).
8. **ENUMERATED and NULL are not checked (H43, H45).** `0A 02 00 01`, `05 01 00` accepted even in DER **[probed]**.
   §8.4 ¶1, §8.8.2 ¶1.
9. **OID arcs and long-form length octet counts are capped below the standard (H13, H52).** §8.19.2 puts no
   size limit on a subidentifier (UUID-based `2.25.x` arcs are 128 bits); §8.1.3.5 allows up to 126 length
   octets and padding with leading zeros. asn1core refuses arcs over 2**32 and counts over 8 **[probed]**.
10. **Wrong rationale recorded for `04 8F 01` (H13).** VECTORS and `test_reserved_length_form_is_refused`
    say a low nibble of 15 is reserved. Only 0xFF is (§8.1.3.5 ¶2 c). 0x8F means 15 length octets; the input
    is refused because it is truncated and because of the 8-octet cap.
11. **BMPString accepts surrogate pairs (H66).** X.680 §41.15 ¶1 limits BMPString to the BMP, excluding the
    cells used for addressing beyond it; §8.23.8 ¶1-2 specifies the 2-octet form with no signature.
12. **Stale statements about leap seconds (H57).** `time.py` docstring and
    `test_a_leap_second_is_refused_and_says_so...` say X.690 permits `ss = 60` and keep a marker. X.680 §47.3
    ¶2 b 2 bounds UTCTime seconds at 00-59. GeneralizedTime remains open (§46.3 defers to ISO 8601).
13. **Constructed strings are parsed but not reassembled (H31).** `content()` of a constructed OCTET STRING returns
    child TLV bytes. §7.3 ¶2 requires a BER receiver to support the constructed form.

## 2. Policy choices (the standard does not decide)

- Ceilings: tag number 2**32 (H05), content length 64 MiB (H14), depth 64 (H23), OID arcs 64 (H52).
- Trailing octets: `decode` returns the end offset, `validate_der` refuses (H24).
- Century window and the 2049/2050 rule: RFC 5280, not X.680 (H56, H62); validity encoding reported, not enforced.
- Calendar validity of dates, year range limited by Python `datetime` (H61, H62).
- Enforce the PrintableString alphabet on the DER path only; defects-as-data in lenient mode (H63, H82).
  NumericString / IA5 / Visible alphabets not checked (H64).
- 8-bit string types decoded as Latin-1, ambiguous ones as UTF-8-then-Latin-1 (H67).
- DER named-bit trailing-zero rule applied by typed decoders, not the generic decoder (H48; correct per §11.2.2 ¶1).
- First DER violation only (H83); BER vs DER as separate named modes (H81).

## 3. The nine retired `[verify against X.690/X.680]` markers

No literal marker remains on any VECTORS row (three descriptive mentions only), so the nine cannot be
listed from the file; PLAN says nine were retired on 2026-11-07. I identified the rows that cite a clause
as read on that date and re-read each clause myself.

| Row | Claim in VECTORS | Clause I read | Result |
|---|---|---|---|
| `tag-nonminimal` | 8.1.2.4.2 c forbids a leading 0x80; "BER permits it" | §8.1.2.4.2 ¶1 c | **Clause resolved, BER claim contradicted**: the shall is in clause 8, BER included. DER enforcement incomplete (H04) |
| `len-nonminimal`, `len-leading-zero` | §10.1 requires minimum octets; BER permits | §10.1 ¶1; §8.1.3.5 ¶5 NOTE 2 | **Resolved**, agrees |
| `len-indefinite` (primitive case) | §8.1.3.2 a, §7.1 gives no licence to accept | §8.1.3.2 ¶1 a; §7.1 ¶1; §7.3 ¶2 | **Resolved** by inference (§7.1 itself says only "clauses 8 to 12"; the argument really rests on §7.3 ¶2) |
| `int-nonmin` | §8.3.2 forbids; "1 in BER" | §8.3.2 ¶1-2 | **Clause resolved, BER claim contradicted**: not valid BER |
| `bool-nonzero` | §8.2.2 any non-zero (sender's option); §11.1 DER all ones | §8.2.2 ¶3; §11.1 ¶1 | **Resolved**, agrees |
| `oid-first-arc-high` | §8.19.4: `7F` is 2.47 | §8.19.4 ¶1-3 | **Resolved**, agrees; the arc limits are X.660 §7.6 ¶1 (read 2026-11-13) |
| `setof-unsorted` | §11.6, plain sort is equivalent | §11.6 ¶1-2 | **Resolved**, agrees for SET OF. Does not cover SET (§10.3), which the code conflates |
| `time-leap-second` | X.680 §47.3: UTCTime ss is 00-59 | X.680 §47.3 ¶2 | **Resolved for UTCTime**. GeneralizedTime still **open** |
| `printable-charset` | X.680 §41.4 Table 10, 74 chars | X.680 §41.4 ¶2-5 | **Resolved**, agrees (26+26+10+1+11) |

Other VECTORS claims I re-checked: `bits-unused-nonzero` (§11.2.1, agrees); `oid-arc-nonminimal` (§8.19.2 ¶6:
**contradicted** as to "BER accept"); `utc-century-rule` (not in X.680 §47.3, RFC 5280 only: agrees with the
row); `time-not-zulu` (X.680 §47.3 ¶2 b, c and §11.8.1-2: agrees); `dn-attribute-tag-numbers`, `dn-teletex-ambiguity`,
`unrestricted-string-value` (Table 1 and Table 8: agree); `numeric-charset` (Table 9: agrees); `time-invalid-calendar`
(§47.3 states only DD 01-31; the calendar rule is inference); `len-undefined-high` (**contradicted**, see 1.10);
`tag-a-9999` arithmetic (9999 = 78*128 + 15: agrees with §8.1.2.4.2). Still open: GeneralizedTime seconds 60 (ISO 8601), constructed time types under DER (§10.2 does not name them).

## 4. What the standard requires that asn1core does not cover

- Reassembly of constructed BIT STRING / OCTET STRING / character strings (§8.6.3, §8.6.4.1, §8.7.3, §8.23.6).
- REAL (§8.5, §11.3), TIME and the useful time types of X.680 clause 38 (§8.26, §11.9), DATE, TIME-OF-DAY,
  DATE-TIME, DURATION (universal 31-34), RELATIVE-OID (§8.20), OID-IRI types (§8.21, §8.22), EXTERNAL,
  EMBEDDED PDV, CHARACTER STRING (§8.16-8.18, §8.24), OBJECT DESCRIPTOR (value checks), CHOICE and open types
  (§8.13, §8.15) in any schema sense.
- GeneralizedTime BER forms (local time, offsets, hour/minute-only, minute/hour fractions, comma) (X.680 §46.3) and
  UTCTime semantics beyond DER form (offsets applied) (X.680 §47.3).
- Value-level BER rules (see 1.3), form-vs-type checks (1.7), stray EOC (1.6).
- A schema layer (DEFAULT omission §11.5, tagging §8.14, SET component order §10.3, SEQUENCE component order §8.9.2).
- Encoders: only `encode_der(node)` re-serialisation, tag/length/OID/time primitives; no builder (ROADMAP `dk.build`).
- CER (clause 9) deliberately absent.
- Large-input features: streaming, arbitrary-size integers/arcs, caller-set limits.
