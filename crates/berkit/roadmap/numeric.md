# numeric: BOOLEAN, INTEGER, ENUMERATED, NULL, REAL

Content-level only: each function takes the contents octets of one value and a `Mode`. The identifier, the form
bit (`wrong_form`), the length and the indefinite form belong to the framing module (obligations BOOL1, INT1,
ENUM2, NULL1, REAL1 and G1 to G5 of `docs/design/primitives.md`), so they are not rows here. Tests are in
`crates/berkit/tests/numeric.rs`; `[ok]` rows each failed under a deliberate breakage (B01 to B18, recorded in
the commit report). Error classes follow the corpus list: the notes' `BadLength`, `RealReserved` are `bad_contents`,
`NonMinimal` is `non_minimal_integer`, `NonCanonical` is `boolean_not_ff` or `real_form`.

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | BOOLEAN contents are exactly one octet; other lengths are `bad_contents` | X.690 §8.2.1 | `boolean_wrong_length_is_bad_contents_in_both_modes` |
| [ok] | `00` is FALSE | X.690 §8.2.2 | `boolean_false_is_00_in_both_modes` |
| [ok] | BER: every non-zero octet is TRUE | X.690 §8.2.2 | `boolean_ber_accepts_every_nonzero_octet_as_true` |
| [ok] | DER: TRUE is `FF`; any other non-zero octet is `boolean_not_ff` | X.690 §11.1 | `boolean_der_rejects_every_nonzero_octet_but_ff` |
| [ok] | BOOLEAN `FF` is accepted in both modes | X.690 §8.2.2, §11.1 | `boolean_true_ff_is_accepted_in_both_modes` |
| [ok] | BOOLEAN encoder writes `FF` or `00` | X.690 §8.2.2, §11.1 | `boolean_encode_writes_literal_ff_and_00` |
| [ok] | INTEGER contents are never empty | X.690 §8.3.1 | `integer_empty_contents_is_bad_contents` |
| [ok] | INTEGER contents are big-endian two's complement; bit 8 of the first octet is the sign | X.690 §8.3.3 | `integer_vectors_decode_to_their_values_in_both_modes` |
| [ok] | First octet plus bit 8 of the second not all ones and not all zero: `00 xx` (xx < 80) and `FF xx` (xx >= 80) are `non_minimal_integer` in BER and DER | X.690 §8.3.2 | `integer_non_minimal_forms_are_rejected_in_ber_and_der` |
| [ok] | `00 80`, `00 FF`, `FF 7F`, `FF 00`, `01 00` are minimal | X.690 §8.3.2 | `integer_boundary_pairs_that_look_non_minimal_are_minimal` |
| [ok] | Encoders emit the minimal form (i64, u64, i128) | X.690 §8.3.2 | `integer_encode_i64_literal_wire_bytes`, `integer_encode_u64_literal_wire_bytes`, `integer_never_emits_a_non_minimal_encoding` |
| [ok] | Encoder from a big-endian magnitude and a sign is minimal; zero has no sign | X.690 §8.3.2, §8.3.3 | `integer_encode_magnitude_signs_and_minimality` |
| [ok] | No size limit: arbitrary-length contents are carried and never truncated; fixed-width conversions return `None` on overflow | X.680 §3.8.48 (INT5) | `integer_300_octets_is_carried_without_truncation`, `integer_9_octets_over_u64_and_i64_do_not_fit`, `integer_17_octets_do_not_fit_i128_or_u128` |
| [ok] | `to_i64` and `to_u64` at the 64-bit edges and for negatives | X.690 §8.3.3 | `integer_conversions_at_the_i64_edges`, `integer_conversions_at_the_u64_edges` |
| [ok] | ENUMERATED contents are those of the associated INTEGER (same functions) | X.690 §8.4 | `integer_vectors_decode_to_their_values_in_both_modes` (V-E01 to V-E03), `integer_non_minimal_forms_are_rejected_in_ber_and_der` (V-E05) |
| [ok] | NULL contents are empty; otherwise `bad_contents` | X.690 §8.8.2 | `null_empty_contents_decode_in_both_modes`, `null_with_contents_is_bad_contents` |
| [ok] | NULL encoder writes zero octets | X.690 §8.8.2 | `null_encodes_to_zero_octets` |
| [ok] | REAL empty contents are plus zero | X.690 §8.5.2 | `real_plus_zero_is_empty_contents` |
| [ok] | Bits 8-7 `01`: one octet, `40` to `43` are the special values and minus zero; others and longer contents are `bad_contents` | X.690 §8.5.9, §8.5.3 | `real_special_values_classify`, `real_reserved_special_values_and_lengths_are_bad_contents`, `real_special_encoders_write_literal_octets` |
| [ok] | Binary: bit 7 sign, bits 6-5 base (`11` reserved), bits 4-3 F, bits 2-1 exponent format; fields split without loss | X.690 §8.5.7 to §8.5.7.5 | `real_binary_fields_are_split_without_loss`, `real_reserved_binary_fields_are_bad_contents_in_both_modes` |
| [ok] | Exponent format `11`: X at least 1; with X >= 2 the first nine bits are not all equal; implied octets must be present | X.690 §8.5.7.4 | `real_reserved_binary_fields_are_bad_contents_in_both_modes` |
| [ok] | Decimal: bits 6-1 `000001`, `000010`, `000011` classify; others are `bad_contents` | X.690 §8.5.8 | `real_decimal_forms_classify`, `real_reserved_decimal_forms_are_bad_contents` |
| [ok] | Contents re-encode byte for byte; no normalisation | X.690 §8.5.7.5 NOTE 1, §7.3 | `real_decode_then_write_is_byte_for_byte` |
| [ok] | DER binary: base 2, F = 0, N odd and without a leading `00`, exponent in the fewest octets (`real_form`) | X.690 §11.3.1 | `real_binary_non_normalised_forms_are_valid_ber_and_real_form_in_der`, `real_der_accepts_canonical_binary` |
| [ok] | DER decimal: NR3 only | X.690 §11.3.2.1 | `real_der_requires_nr3` |
| [ok] | DER NR3 text: no SPACE, leading `-` only if negative, first and last mantissa digit not 0, `.` then `E`, exponent `+0` or no plus and no leading zero | X.690 §11.3.2.2 to §11.3.2.6 | `real_der_nr3_syntax` |
| [ok] | POLICY (derived, not stated): DER refuses exponent format `11` when X < 4 (a shorter format exists, one value one encoding) | X.690 §7.4, §11.3.1 ("fewest octets") | `real_binary_non_normalised_forms_are_valid_ber_and_real_form_in_der` (cases "form 11, X=1" and "X=3") |
| [open] | REAL exponent format `11` with X = 1: the "first nine bits" cannot be examined; the standard is silent. Policy: BER accepts, carried opaquely; DER refuses (X < 4) | X.690 §8.5.7.4 d), notes section 4 item 8 | `real_form_11_with_one_exponent_octet_is_accepted_in_ber_open_row` |
| [open] | Binary REAL with no mantissa octets: not stated. Policy: BER accepts, DER refuses (N not odd) | notes section 4 item 8 | `real_binary_non_normalised_forms_are_valid_ber_and_real_form_in_der` (case "no mantissa") |
| [open] | No conversion of REAL to or from floating point; no decimal parsing; no base conversion (deferred by the REAL scope decision) | notes section 5 | none |
| [unread] | Decimal field grammar in BER (NR1, NR2, NR3 digit, sign and decimal-mark syntax; whether an empty field is a value) and the NR3 character set in DER (digits only is assumed; a comma decimal mark is refused) | ISO 6093, not in the corpus | `real_der_nr3_syntax` (passes on the §11.3.2 rules only) |
| [unread] | Whether `-` in the exponent of a DER NR3 is the only sign besides the stated `+0` | ISO 6093 and X.690 §11.3.2.6 | `real_der_nr3_syntax` |
| [ok] | Not found in X.690: a maximum INTEGER size. None is imposed | notes section 4 item 1 | `integer_300_octets_is_carried_without_truncation` |
| [ok] | Not found in X.690: a BER receiver rule rejecting a non-minimal INTEGER. POLICY: rejected in BER too, because §8.3.2 is a "shall" with no sender's option | notes section 4 item 5; ROADMAP policy 1 | `integer_non_minimal_forms_are_rejected_in_ber_and_der` |
| [ok] | Not found in X.690: a rule rejecting a BOOLEAN of the wrong length; derived from §8.2.1 | notes section 4 | `boolean_wrong_length_is_bad_contents_in_both_modes` |
| [open] | Not found in X.690: how a decoder treats ISO 6093 scaling factors in documentation (§8.5.8 forbids them; nothing to check at octet level) | X.690 §8.5.8 ¶4 | none |
