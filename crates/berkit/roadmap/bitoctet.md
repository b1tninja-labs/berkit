# bitstring and octetstring roadmap

Tests are in `crates/berkit/tests/strings_bits.rs`. `[ok]` means the test exists, passes, and failed under a
deliberate breakage of the code it covers. Offsets count from the start of the contents (for a constructed
form, the index of the segment).

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | BIT STRING primitive contents: initial octet then data; empty contents is `bad_contents` | X.690 §8.6.2 | `empty_contents_has_no_initial_octet` |
| [ok] | Bits are placed leading bit first into bit 8 downwards; bit length excludes the initial octet | X.690 §8.6.2.1 | `zero_unused_bits_whole_octet`, `annex_value_primitive` |
| [ok] | Initial octet is the unused-bit count, 0 to 7; above 7 is `bad_contents` | X.690 §8.6.2.2 | `initial_octet_above_seven_is_bad_contents`, `seven_unused_bits_leaves_one_bit` |
| [ok] | Empty value is initial octet 0 and no data; a non-zero initial octet with no data is `bad_contents` (derived from §8.6.2.2 with §8.6.2.3) | X.690 §8.6.2.3 | `empty_value_is_a_single_zero_octet`, `unused_bits_with_no_data_octets_is_bad_contents` |
| [ok] | DER: unused bits are zero, always, `unused_bits_nonzero` | X.690 §11.2.1 | `nonzero_pad_bits_are_accepted_in_ber_and_refused_in_der` |
| [ok] | DER: trailing 0 bits removed, only where the type has a NamedBitList; the caller sets `named_bit_list`; the all-zero value is `00` | X.690 §11.2.2, X.680 §22.7 | `named_bit_list_in_der`, `named_bit_list_value_with_and_without_trailing_zeros` |
| [ok] | Encoder: unused bits always written as zero; encoder helper for the NamedBitList length | X.690 §8.6.2.2, §11.2.1, §11.2.2 | `encode_writes_literal_contents`, `named_bit_list_len_strips_trailing_zero_bits` |
| [ok] | BER does not constrain the values of the unused bits (no rule was found in X.690; POLICY: accept, and equality ignores them) | X.690 §8.6.2 (silent), primitives.md s4 items 4 and 9 | `nonzero_pad_bits_are_accepted_in_ber_and_refused_in_der`, `unequal_values_are_not_equal` |
| [ok] | BER does not require trailing-zero removal and `named_bit_list` has no effect in BER | X.680 §22.7 | `named_bit_list_in_der` (V-S22 part) |
| [ok] | Constructed BIT STRING: BER only; DER `constructed_string` | X.690 §8.6.1, §10.2 | `constructed_acceptable_by_mode`, `annex_value_constructed` |
| [ok] | Constructed: zero segments give the empty value; empty segments allowed | X.690 §8.6.3, §8.6.4 NOTE | `constructed_zero_segments_is_the_empty_value`, `constructed_empty_segments_are_allowed` |
| [ok] | Constructed: every segment but the last has zero unused bits, else `bad_contents` | X.690 §8.6.4 ¶2 | `constructed_non_final_segment_with_unused_bits_is_bad_contents` |
| [ok] | Constructed: segments concatenate in order; totals and in-order data and bit iterators | X.690 §8.6.4 ¶3, §8.6.4.1 ¶3 | `annex_value_constructed`, `constructed_final_pad_bits_are_masked_on_reassembly` |
| [ok] | Constructed nesting is flattened by the caller; the flat rule matches the recursive rule | X.690 §8.6.4.1 ¶1-¶4 | `constructed_nested_segments_flattened_by_the_caller` |
| [open] | Inner segment tag is universal 3 (V-S19); needs the identifier octets, so it belongs to the TLV layer | X.690 §8.6.4.1 NOTE 2 | none here |
| [open] | Definite and indefinite length of the constructed encoding (BIT14) | X.690 §8.1.3.2, §8.1.5 | none here (TLV layer) |
| [ok] | OCTET STRING primitive: the contents are the octets; empty allowed; encode copies | X.690 §8.7.2 | `primitive_octet_string` |
| [ok] | OCTET STRING constructed: BER only, zero segments empty, empty segments allowed, concatenation in order; DER `constructed_string` | X.690 §8.7.1, §8.7.3, §8.7.3.1, §10.2 | `constructed_octet_string_concatenates`, `constructed_octet_string_nested_empty_and_none`, `constructed_acceptable_by_mode` |
| [open] | Inner segment tag is universal 4 (V-O06); TLV layer | X.690 §8.7.3.2 NOTE 2 | none here |
| [open] | Policy: an output buffer that is too small reports `limit_exceeded`; no corpus class names it | none (POLICY) | `encode_refuses_short_input_and_short_output`, `primitive_octet_string` |
| [open] | Policy: `bad_contents` for the three §8.6.2 breaks, in place of the notes' BadLength, BitStringUnusedRange, BitStringUnusedEmpty, BitStringSegment; `constructed_string` in place of WrongForm for DER constructed | corpus/format.md error classes | the tests above |
| [open] | Not found in X.690: values of unused bits in BER (primitives.md s4 item 4) and BER equality of such values (item 9): policy above | none | `unequal_values_are_not_equal` |
| [open] | Not found in X.690: any limit on constructed nesting depth or segment count (item 12); this module has none and the caller bounds the flattening | none | none |
| [open] | Not found in X.690: how a nested constructed non-final segment's unused bits are judged (item 13); chosen: flattened, every non-final leaf has zero unused bits | X.690 §8.6.4.1 (implied) | `constructed_nested_segments_flattened_by_the_caller` |
