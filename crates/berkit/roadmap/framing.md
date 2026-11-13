# Framing roadmap (tag.rs, length.rs, tlv.rs)

Anchors are taken from `docs/design/framing.md`. The `itulib` index was unavailable in this session
(`itu_read` returned "no such table: meta" for 8.1.3.5, 8.1.5, 8.1.2.4.2 and 10.1), so no anchor was re-read from
X.690 by the author of this module. A row whose test passes and failed under a breakage but whose anchor is not
re-read is `[unread]`; it becomes `[ok]` when someone reads the clause. `B nn` names a deliberate breakage
(all restored and verified with `diff`); tests are in `crates/berkit/tests/framing.rs`.

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [unread] | F-01 BER accepts every sender alternative (non-minimal long forms, indefinite, constructed) | X.690 §7.3 ¶2 | `tlv_non_minimal_length_is_kept_and_der_rejects_it`, `indefinite_forms_ber`; B03 |
| [unread] | F-02, F-41 DER: definite form only, minimal length octets | §7.4, §10.1 ¶1 | `length_non_minimal_long_forms_ber_accepts_der_rejects`, `indefinite_der_is_refused_everywhere_by_header`; B05, B10, B20 |
| [unread] | F-03 components in order: identifier, length, contents, EOC | §8.1.1.1 ¶1 | `writer_emits_literal_wire_bytes`, `truncation_sweep_*`; B13, B18, B24 |
| [unread] | F-04 EOC only after an indefinite length; `00 00` after a definite value is trailing data | §8.1.1.2 | `eoc_errors`; B06, B17 |
| [unread] | F-05, F-06 identifier carries class (all four) and number | §8.1.2.1, §8.1.2.2 Table 1 | `tag_vectors_decode_and_encode_to_the_literal_octets`, `tag_class_bits_round_trip`; B21 |
| [unread] | F-07 bit 6 is the constructed bit | §8.1.2.2 b), §8.1.2.5 | `tag_vectors_decode_and_encode_to_the_literal_octets`; B22 |
| [unread] | F-08 numbers 0..30 in one octet | §8.1.2.2 c) | `tag_vectors_decode_and_encode_to_the_literal_octets`; B23 |
| [unread] | F-09, F-10, F-11 high form: leading octet `xx111111`, continuation bit 8, truncation | §8.1.2.4, §8.1.2.4.1, §8.1.2.4.2 a) | `tag_vectors_...`, `tag_errors`, `truncation_sweep_b_*`; B18 |
| [unread] | F-12 base-128 big-endian number | §8.1.2.4.2 b) | `tag_vectors_...` (30, 31, 127, 128, 16383, 16384, 2^32, 2^64-1); B18 |
| [unread] | F-13 leading zero subsequent octet is `non_minimal_tag` | §8.1.2.4.2 c) | `tag_errors`; B04 |
| [unread] | F-14 number 0..30 in the high form is `non_minimal_tag` (derived) | §8.1.2.2 with §8.1.2.4 | `tag_errors`; B07 |
| [unread] | F-16 Figure 4 after Erratum 1: penultimate subsequent octet has bit 8 = 1 | §8.1.2.4.3, Erratum 1 | `tag_vectors_...` (literal bytes); B18 |
| [open] | F-15, F-36, F-37, F-38, F-42 constructed-versus-type, tagging, constructed strings, CHOICE, DER constructed strings | §8.1.2.5, §8.14, §8.6.1, §10.2 | typed layer; at framing the bit is data (`tlv_vectors_ok_in_both_modes` checks it is carried) |
| [unread] | F-17 tag number is a `u64`; above it is `limit_exceeded` (POLICY: the standard sets no maximum) | X.680 §8.2; not found in X.690 | `tag_errors`, `header_and_extent_errors`; B08 |
| [open] | F-18 UNIVERSAL 0 only as the closing EOC; unassigned universal numbers (15, 37+) accepted at framing (POLICY) | X.680 §8.4 Table 1 | `eoc_errors`, `tlv_unassigned_universal_numbers_pass_at_framing`; B06 proves the EOC part, no breakage for the 15/37 policy |
| [unread] | F-19, F-22, F-23 definite short and long forms | §8.1.3.1, §8.1.3.3, §8.1.3.4 | `length_vectors_ber_and_der` (0, 127, 128, 201, 255, 256, 65535, 65536); B13, B24 |
| [unread] | F-20, F-33 indefinite on a primitive is `indefinite_on_primitive`, decided from the identifier, in both modes | §8.1.3.2 a), §8.1.3.6 | `indefinite_on_primitive_is_decided_by_the_identifier`; B09 |
| [unread] | F-21, F-29 indefinite form `80` on constructed in BER; `81 00` is a long-form zero | §8.1.3.2 b), §8.1.3.6.1 | `length_indefinite_is_a_single_80_and_81_00_is_zero`; B02 |
| [unread] | F-25, F-27 long form, big-endian value | §8.1.3.5 ¶1-4 | `length_vectors_ber_and_der`; B24 |
| [unread] | F-26 initial octet `FF` is `reserved_length` | §8.1.3.5 ¶2 c) | `length_errors`, `header_and_extent_errors`; B01 |
| [unread] | F-28 BER accepts leading zero length octets and counts above 8 when the value fits; limit applies to the value | §8.1.3.5 NOTE 2 | `length_non_minimal_long_forms_ber_accepts_der_rejects` (9 and 127 octets); B03 |
| [unread] | F-30, F-31 indefinite contents end at the first EOC at their level; EOC is exactly `00 00`; missing EOC is `missing_eoc` | §8.1.3.6.2, §8.1.5 ¶1-2 | `indefinite_forms_ber`, `eoc_errors`, `truncation_sweep_c_indefinite`; B15, B16 |
| [unread] | F-35 a child that does not fit a definite parent is `length_overrun`, header or contents | §8.1.3.3, §8.1.4 | `header_and_extent_errors`, `truncation_sweep_inside_a_definite_parent_*`; B12 |
| [unread] | F-44 DER adds no tag rule beyond BER | §10 | `header_and_extent_errors` runs the tag cases in both modes |
| [open] | F-24, F-32, F-34, F-43 informative or out of scope (short-form note, EOC as tag 0 note, contents per type, CER) | §8.1.3.3 NOTE, §8.1.5 ¶3, §8.1.4, §9.1 | no behaviour of its own |
| [unread] | Truncation: every proper prefix refused (A: 8, B: 262 incl. high tag and long length, C: 7 incl. indefinite, Annex A record: 136) | §8.1.1.1 | `truncation_sweep_*`, `annex_a_record_parses_and_every_prefix_is_refused`; B15 |
| [unread] | Stream order: first fault wins, header before contents, depth first, fault inside before trailing data | `corpus/format.md` | `stream_order_reports_the_first_fault`; B19, B20 |
| [unread] | Exact slices: header, content and whole encoding are the input octets; offsets absolute | X.690 §11.6 needs them (DER SET OF) | `tlv_slices_are_the_exact_input_octets`, `children_iterate_without_allocation_and_keep_offsets`; B14 |
| [unread] | Writer: minimal tag and length, literal wire bytes, refuses UNIVERSAL 0, indefinite primitive and a short buffer | §8.1.2, §8.1.3, §10.1 | `writer_emits_literal_wire_bytes`, `writer_refusals`, `round_trip_writer_to_reader_with_literal_check`; B13, B18, B26, B27 |
| [open] | POLICY: nesting limit, default 64 levels (top level is 1), clamped to 512 whatever the caller asks; `limit_exceeded` | not found in X.690 (§6 item 4 of the notes) | `depth_limit_is_limit_exceeded_not_a_crash`; B11 |
| [open] | POLICY: contents length limit, default 256 MiB; checked after the length is compared with the octets present, so a length beyond the input is `truncated` or `length_overrun` first | not found in X.690 (item 3) | `content_limit`; B25 |
| [open] | POLICY: a length above `u64::MAX` is `limit_exceeded` | not found in X.690 (item 3) | `length_errors`; no breakage run |
| [open] | POLICY: `parse` returns the rest; `parse_exact` reports `trailing_data` after validating the whole tree | not found in X.690 (item 8) | `trailing_data_via_parse_exact`; B17 |
| [open] | POLICY: stray EOC and tag-0 not `00 00` are reported as `bad_contents` because the shared class list has no class for them (constant `tlv::STRAY_EOC`) | X.690 §8.1.1.2, §8.1.5; X.680 Table 1 | `eoc_errors`; B06, B16. Needs a shared `ErrorClass` (coordinator) |
| [open] | The `Item::Eoc` arm in `Children::next` is unreachable (an EOC is never read inside a region that is not an open indefinite one); breakage B28 on it survives | n/a | none |
| [open] | No streaming, no CER, no checking that `max_depth` of a caller-built `Limits` is sensible | n/a | none |
