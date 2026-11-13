# Corrections to the research notes

Each entry: the case id, what the note said, what the clause says, the clause. Every item was found by re-deriving the
bytes by hand and by running them through `tools/refdec.py`. Where a note's vector was wrong the case carries the
corrected bytes.

## Vectors that were wrong

1. `x690.8.19.oid-truncated-subid`. primitives.md V-D18 had `06 03 2A 86 48 86`: the length octet says 3 but the contents
   `2A 86 48 86` are 4 octets, so the input fails on length before the OID rule is reached. Corrected to
   `06 04 2A 86 48 86`: the last octet 86 has bit 8 set, so the subidentifier never ends. Clause: X.690 §8.19.2 clause 3.
2. `x690.8.20.relative-oid-truncated`. primitives.md V-R05 had `0D 01 80`: the single octet 80 is both a subidentifier that starts
   with 80 (non-minimal, §8.20.2 clause 6) and one that never ends (clause 3); stream order reports the first. Replaced by
   `0D 01 81`, which has only the second fault. Clause: X.690 §8.20.2 clause 3.
3. `x690.11.6.setof-same-tag-length-order-wrong` and `x690.11.6.setof-same-tag-length-order-right`. structured.md had
   `31 0A ...` for two SET OF vectors whose contents are 4 + 4 + 3 = 11 octets. Corrected to `31 0B`. With `0A` the last
   element overruns its parent and the case would test the wrong rule. Clause: X.690 §8.1.3.4 (length), §11.6 (order).

## Notes that disagreed with the format's classes, or with each other

4. `x690.8.3.integer-ff7f-minus-129`. The original brief called `FF 7F` non-minimal. primitives.md V-I05 and X.690 §8.3.2 say it is
   minimal: the first octet (all ones) and bit 8 of the second octet (0 for `7F`) are not all ones, so rule a) does not trigger.
   It is -129. The non-minimal two-octet forms are `00 xx` with xx < 80 and `FF xx` with xx >= 80
   (`x690.8.3.integer-nonminimal-*`).
5. Missing EOC. framing.md and structured.md name `E_TRUNCATED` for an indefinite-length encoding that ends at an element
   boundary without its EOC (`30 80`, `30 80 02 01 05`, `30 80 30 80 00 00` and the prefixes of `30 80 02 01 05 00 00`).
   `format.md` defines `missing_eoc` for exactly this (X.690 §8.1.3.6.2, §8.1.5 clause 1), so the cases say `missing_eoc`.
   Ids: `x690.8.1.3.6.indefinite-eoc-missing`, `...-nested-outer-eoc-missing`, `x690.8.1.3.6.truncated-prefix-c-2` and `-c-5`.
   Where the input ends inside an element (a child's identifier or length, `30 80 02`, `30 80 02 01`) it stays `truncated`.
6. A lone `00` where an EOC is due. framing.md gave `30 80 02 01 05 00` as E_TRUNCATED and `30 03 30 80 00` as E_TRUNCATED
   ("length_overrun also reasonable"). The `00` is the identifier of an element whose length octet is missing: `truncated`
   when the region ends with the input (`x690.8.1.3.6.indefinite-half-eoc`, `...truncated-prefix-c-6`), `length_overrun` when
   the region is the contents of a definite-length parent (`x690.8.1.3.6.definite-parent-indefinite-child-eoc-cut`).
   Clauses: X.690 §8.1.3.3, §8.1.4, §8.1.5 clause 2. (An earlier draft of this corpus said `missing_eoc` for both; the berkit
   framing implementation and the coordinator's review gave the rule above.)
7. DER and constructed strings. framing.md gave `E_INDEFINITE_FORBIDDEN` for `23 80 ...` and `3A 80 ...` in DER, and primitives.md
   gave `WrongForm` for `23 ...` and `24 ...`. By the stream-order rule the identifier is judged before the length, and
   `constructed_string` (X.690 §10.2) is the class for a constructed string in DER (format.md). The cases are typed and say
   `constructed_string` under `der`: `x690.8.6.bitstring-constructed-*`, `x690.8.7.octetstring-constructed-*`,
   `x690.8.23.visiblestring-constructed-*`, `x690.8.25.*-constructed`. Untyped, `30 80 ...` is still `indefinite_length` in DER.
8. Stray EOC and tag 0. framing.md `E_EOC_MISPLACED` and `E_TAG0_NOT_EOC` and structured.md `UnexpectedEoc` had no class among
   the first 23; they are `stray_eoc` (CHANGES.md). `30 80 00 01 05` is `stray_eoc` in BER and `indefinite_length` in DER (the
   length octet of the first element comes first).
9. Provenance of two sample cases. `sample-cases.json` said `from X.690 §11.7.5 (Example)` for the GeneralizedTime and UTCTime
   examples; the notes (strings-time.md) say `by hand from X.690 §11.7.5`: the standard prints the string, not the TLV. The
   corpus keeps the notes' wording (`constructed by hand`). Their expectations now also name BER (accepts), which the samples
   left open.

## Class mapping of the notes' own names (not corrections)

primitives.md and strings-time.md use their own names; the corpus uses the 24 of `format.md`.
`BadLength`, `BitStringUnusedRange`, `BitStringUnusedEmpty`, `BitStringSegment`, `OidTruncated`, `OidEmpty`, `RealReserved`,
`InvalidUtf8`, `ALPHA`, `CODING`, `TSYNTAX`, `TRANGE` and a wrong segment tag: `bad_contents`. `NonMinimal`:
`non_minimal_integer`, `non_minimal_subidentifier` or `non_minimal_length` by the thing that is not minimal. `NonCanonical`:
`boolean_not_ff`, `unused_bits_nonzero`, `bitstring_trailing_zeros` or `real_form` by the rule. `DERFORM` for times:
`time_format`; for constructed strings: `constructed_string`. `E_LIMIT_TAG`, `E_LIMIT_LEN`: `limit_exceeded`.
`E_TAG_LEADING_ZERO`, `E_TAG_HIGH_FORM_FOR_LOW_NUMBER`: `non_minimal_tag`.
