# Framing roadmap (tag.rs, length.rs, tlv.rs)

Anchors were first taken from `docs/design/framing.md` because the `itulib` index was unavailable to the author of this
module. On 2026-11-13 the clauses were re-read from X.690 (02/2021) through `primarysource itu read` (see "Clause reading"
below): 22 rows agree with the clause and a named test fails under a fresh breakage, so they are `[ok]`; four stay
`[unread]` for the reasons given there. `B nn` names an earlier deliberate breakage (all restored and verified with
`diff`; their definitions are not kept in the repository, which is why the section below records new ones);
tests are in `crates/berkit/tests/framing.rs`.

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | F-01 BER accepts every sender alternative (non-minimal long forms, indefinite, constructed) | X.690 §7.3 ¶2 | `tlv_non_minimal_length_is_kept_and_der_rejects_it`, `indefinite_forms_ber`; B03 |
| [ok] | F-02, F-41 DER: definite form only, minimal length octets | §7.4, §10.1 ¶1 | `length_non_minimal_long_forms_ber_accepts_der_rejects`, `indefinite_der_is_refused_everywhere_by_header`; B05, B10, B20 |
| [ok] | F-03 components in order: identifier, length, contents, EOC | §8.1.1.1 ¶1 | `writer_emits_literal_wire_bytes`, `truncation_sweep_*`; B13, B18, B24 |
| [ok] | F-04 EOC only after an indefinite length; `00 00` after a definite value is trailing data | §8.1.1.2 | `eoc_errors`; B06, B17 |
| [ok] | F-05, F-06 identifier carries class (all four) and number | §8.1.2.1, §8.1.2.2 Table 1 | `tag_vectors_decode_and_encode_to_the_literal_octets`, `tag_class_bits_round_trip`; B21 |
| [ok] | F-07 bit 6 is the constructed bit | §8.1.2.2 b), §8.1.2.5 | `tag_vectors_decode_and_encode_to_the_literal_octets`; B22 |
| [ok] | F-08 numbers 0..30 in one octet | §8.1.2.2 c) | `tag_vectors_decode_and_encode_to_the_literal_octets`; B23 |
| [ok] | F-09, F-10, F-11 high form: leading octet `xxx11111` (bits 8 and 7 class, bit 6 constructed, bits 5 to 1 all one), continuation bit 8, truncation | §8.1.2.4, §8.1.2.4.1, §8.1.2.4.2 a) | `tag_vectors_...`, `tag_errors`, `truncation_sweep_b_*`; B18 |
| [ok] | F-12 base-128 big-endian number | §8.1.2.4.2 b) | `tag_vectors_...` (30, 31, 127, 128, 16383, 16384, 2^32, 2^64-1); B18 |
| [ok] | F-13 leading zero subsequent octet is `non_minimal_tag` | §8.1.2.4.2 c) | `tag_errors`; B04 |
| [ok] | F-14 number 0..30 in the high form is `non_minimal_tag` (derived) | §8.1.2.2 with §8.1.2.4 | `tag_errors`; B07 |
| [unread] | F-16 Figure 4 after Erratum 1: penultimate subsequent octet has bit 8 = 1 | §8.1.2.4.3, Erratum 1 | `tag_vectors_...` (literal bytes); B18 |
| [open] | F-15, F-36, F-37, F-38, F-42 constructed-versus-type, tagging, constructed strings, CHOICE, DER constructed strings | §8.1.2.5, §8.14, §8.6.1, §10.2 | typed layer; at framing the bit is data (`tlv_vectors_ok_in_both_modes` checks it is carried) |
| [unread] | F-17 tag number is a `u64`; above it is `limit_exceeded` (POLICY: the standard sets no maximum) | X.680 §8.2; not found in X.690 | `tag_errors`, `header_and_extent_errors`; B08 |
| [open] | F-18 UNIVERSAL 0 only as the closing EOC; unassigned universal numbers (15, 37+) accepted at framing (POLICY) | X.680 §8.4 Table 1 | `eoc_errors`, `tlv_unassigned_universal_numbers_pass_at_framing`; B06 proves the EOC part, no breakage for the 15/37 policy |
| [ok] | F-19, F-22, F-23 definite short and long forms | §8.1.3.1, §8.1.3.3, §8.1.3.4 | `length_vectors_ber_and_der` (0, 127, 128, 201, 255, 256, 65535, 65536); B13, B24 |
| [ok] | F-20, F-33 indefinite on a primitive is `indefinite_on_primitive`, decided from the identifier, in both modes | §8.1.3.2 a), §8.1.3.6 | `indefinite_on_primitive_is_decided_by_the_identifier`; B09 |
| [ok] | F-21, F-29 indefinite form `80` on constructed in BER; `81 00` is a long-form zero | §8.1.3.2 b), §8.1.3.6.1 | `length_indefinite_is_a_single_80_and_81_00_is_zero`; B02 |
| [ok] | F-25, F-27 long form, big-endian value | §8.1.3.5 ¶1-4 | `length_vectors_ber_and_der`; B24 |
| [ok] | F-26 initial octet `FF` is `reserved_length` | §8.1.3.5 ¶2 c) | `length_errors`, `header_and_extent_errors`; B01 |
| [ok] | F-28 BER accepts leading zero length octets and counts above 8 when the value fits; limit applies to the value | §8.1.3.5 NOTE 2 | `length_non_minimal_long_forms_ber_accepts_der_rejects` (9 and 127 octets); B03 |
| [ok] | F-30, F-31 indefinite contents end at the first EOC at their level; EOC is exactly `00 00`; missing EOC is `missing_eoc` | §8.1.3.6.2, §8.1.5 ¶1-2 | `indefinite_forms_ber`, `eoc_errors`, `truncation_sweep_c_indefinite`; B15, B16 |
| [ok] | F-35 a child that does not fit a definite parent is `length_overrun`, header or contents | §8.1.3.3, §8.1.4 | `header_and_extent_errors`, `truncation_sweep_inside_a_definite_parent_*`; B12 |
| [unread] | F-44 DER adds no tag rule beyond BER | §10 | `header_and_extent_errors` runs the tag cases in both modes |
| [open] | F-24, F-32, F-34, F-43 informative or out of scope (short-form note, EOC as tag 0 note, contents per type, CER) | §8.1.3.3 NOTE, §8.1.5 ¶3, §8.1.4, §9.1 | no behaviour of its own |
| [ok] | Truncation: every proper prefix refused (A: 8, B: 262 incl. high tag and long length, C: 7 incl. indefinite, Annex A record: 136) | §8.1.1.1 | `truncation_sweep_*`, `annex_a_record_parses_and_every_prefix_is_refused`; B15 |
| [unread] | Stream order: first fault wins, header before contents, depth first, fault inside before trailing data | `corpus/format.md` | `stream_order_reports_the_first_fault`; B19, B20 |
| [ok] | Exact slices: header, content and whole encoding are the input octets; offsets absolute | X.690 §11.6 needs them (DER SET OF) | `tlv_slices_are_the_exact_input_octets`, `children_iterate_without_allocation_and_keep_offsets`; B14 |
| [ok] | Writer: minimal tag and length, literal wire bytes, refuses UNIVERSAL 0, indefinite primitive and a short buffer | §8.1.2, §8.1.3, §10.1 | `writer_emits_literal_wire_bytes`, `writer_refusals`, `round_trip_writer_to_reader_with_literal_check`; B13, B18, B26, B27 |
| [open] | POLICY: nesting limit, default 64 levels (top level is 1), clamped to 512 whatever the caller asks; `limit_exceeded` | not found in X.690 (§6 item 4 of the notes) | `depth_limit_is_limit_exceeded_not_a_crash`; B11 |
| [open] | POLICY: contents length limit, default 256 MiB; checked after the length is compared with the octets present, so a length beyond the input is `truncated` or `length_overrun` first | not found in X.690 (item 3) | `content_limit`; B25 |
| [open] | POLICY: a length above `u64::MAX` is `limit_exceeded` | not found in X.690 (item 3) | `length_errors`; no breakage run |
| [open] | POLICY: `parse` returns the rest; `parse_exact` reports `trailing_data` after validating the whole tree | not found in X.690 (item 8) | `trailing_data_via_parse_exact`; B17 |
| [open] | POLICY: stray EOC and tag-0 not `00 00` are reported as `bad_contents` because the shared class list has no class for them (constant `tlv::STRAY_EOC`) | X.690 §8.1.1.2, §8.1.5; X.680 Table 1 | `eoc_errors`; B06, B16. Needs a shared `ErrorClass` (coordinator) |
| [open] | The `Item::Eoc` arm in `Children::next` is unreachable (an EOC is never read inside a region that is not an open indefinite one); breakage B28 on it survives | n/a | none |
| [open] | No streaming, no CER, no checking that `max_depth` of a caller-built `Limits` is sensible | n/a | none |

## Long-form lengths in the writer (added with the corpus runner)

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | A BER sender may write a definite length in the long form with surplus, zero-padded length octets, so the writer offers `length::encode_long` and `tlv::write_header_long`; DER takes the minimal form only and `write_header` writes it | X.690 §8.1.3.5 NOTE 2, §10.1 | `tests/framing.rs::a_long_form_length_with_a_chosen_number_of_octets_is_written_exactly`, `a_long_form_length_is_refused_when_it_cannot_be_written`, `a_header_with_a_padded_long_length_reads_back_in_ber_and_is_refused_in_der`, `the_padding_octets_of_a_long_form_length_are_written_into_a_dirty_buffer`; the corpus case `x690.8.1.3.encode-length-nonminimal-81-01` (a break of the marker bit failed 3 tests; a break of the zero padding failed only the dirty-buffer test, which uses nine octets because a `u64` fills the last eight) |

## Clause reading (2026-11-13)

Each clause was read with `python -m primarysource.cli itu read "X.690§..."`. Cites are Rec. ITU-T X.690 (02/2021) as
printed by the tool; the quoted phrase is one per row. A "derived" finding means the clause states only the sender's
rule and the receiver class or behaviour is this crate's reading of it (policy table in `ROADMAP.md`, DER row). Each
breakage was one edit to `crates/berkit/src`, the changed line printed, `cargo clean -p berkit`, `cargo test --test
framing`, the file restored from a copy outside the repository and compared with `diff`; the names after the colon are
the framing tests that failed.

| Row | Clause read | Finding | Breakage and the tests that failed |
|---|---|---|---|
| F-01 | §7.3 ¶2 "shall support all alternatives" | agrees | BER also refuses a non-minimal long form (`tlv.rs`, the DER-only test made unconditional): `tlv_non_minimal_length_is_kept_and_der_rejects_it`, `a_header_with_a_padded_long_length_reads_back_in_ber_and_is_refused_in_der`, `stream_order_reports_the_first_fault`, `tlv_slices_are_the_exact_input_octets` |
| F-02, F-41 | §7.4 ¶1; §10.1 ¶1 "minimum number of octets" | agrees | DER minimal test off in `tlv.rs` (3 failed), in `length::decode` (`length_non_minimal_long_forms_ber_accepts_der_rejects`), DER indefinite test off (`indefinite_der_is_refused_everywhere_by_header`, `eoc_errors`, `truncation_sweep_c_indefinite`, `stream_order_reports_the_first_fault`) |
| F-03 | §8.1.1.1 ¶1 "four components" in order | agrees | writer emits length before identifier: `writer_emits_literal_wire_bytes`, `round_trip_writer_to_reader_with_literal_check` |
| F-04 | §8.1.1.2 "shall not be present unless" | agrees; "`00 00` after a definite value is `trailing_data`" is this crate's policy (the clause is silent) | UNIVERSAL 0 accepted as an element: `eoc_errors`, `a_stray_end_of_contents_is_the_stray_eoc_class`; trailing octets accepted by `parse_exact`: `trailing_data_via_parse_exact`, `eoc_errors` |
| F-05, F-06 | §8.1.2.1; §8.1.2.2 ¶1 a), ¶2 Table 1 | agrees | APPLICATION class bits 1 to 3: `tag_class_bits_round_trip`, `tag_vectors_decode_and_encode_to_the_literal_octets`, `round_trip_writer_to_reader_with_literal_check` |
| F-07 | §8.1.2.5 ¶1 "Bit 6 shall be set to zero" | agrees | constructed bit read from bit 7 (0x40): 13 tests including `tag_vectors_decode_and_encode_to_the_literal_octets` |
| F-08 | §8.1.2.2 ¶1 c) bits 5 to 1 | agrees | five-bit number mask 0x1F to 0x0F: 18 tests including `tag_vectors_decode_and_encode_to_the_literal_octets` |
| F-09, F-10, F-11 | §8.1.2.4; §8.1.2.4.1 c) "111112"; §8.1.2.4.2 a) | row text disagreed: the leading octet is `xxx11111`, not `xx111111` (bit 6 is the constructed bit, §8.1.2.4.1 b)); the code masks `0x1F` and was right. Row text corrected. Truncation (F-11) is derived from a) | continuation bit read from bit 7: `tag_vectors_...`, `tag_errors`, `truncation_sweep_b_high_tag_and_long_length` and 4 more; truncation reported as `non_minimal_tag`: `tag_errors`, `truncation_sweep_b_high_tag_and_long_length`, `header_and_extent_errors`, `children_iterate_without_allocation_and_keep_offsets` |
| F-12 | §8.1.2.4.2 b) base-128, bit 7 of first octet most significant | agrees | shift 7 to 6: `tag_vectors_...`, `tag_errors`, `header_and_extent_errors` and 2 more |
| F-13 | §8.1.2.4.2 c) "shall not all be zero" | agrees | test removed: `tag_errors` |
| F-14 | §8.1.2.2 ¶1 "a single octet" for 0 to 30 | agrees (derived: the clause does not name the error class) | test removed: `tag_errors`, `header_and_extent_errors`, `stream_order_reports_the_first_fault` |
| F-19, F-22, F-23 | §8.1.3.1 ¶2; §8.1.3.3 ¶1-2; §8.1.3.4 ¶1 | agrees | short form ends at 126, not 127, in `Length::encode`: `length_vectors_ber_and_der`, `round_trip_writer_to_reader_with_literal_check` |
| F-20, F-33 | §8.1.3.2 a) "use the definite form" for primitive | agrees, derived: §8.1.3.2 is a sender rule, and §8.1.3.6 (the indefinite form) says nothing about primitive encodings | test removed: `indefinite_on_primitive_is_decided_by_the_identifier` |
| F-21, F-29 | §8.1.3.6 ¶1 "a single octet"; §8.1.3.6.1; §8.1.3.5 ¶4 | agrees (`81 00` is the long form of zero by ¶4) | `81 00` read as indefinite: `length_indefinite_is_a_single_80_and_81_00_is_zero`, `length_non_minimal_long_forms_ber_accepts_der_rejects`, `tlv_non_minimal_length_is_kept_and_der_rejects_it` |
| F-25, F-27 | §8.1.3.5 ¶1, ¶2 a) b), ¶4 | agrees | shift 8 to 7: `length_vectors_ber_and_der`, `length_errors` and 5 more |
| F-26 | §8.1.3.5 ¶2 c) "shall not be used" | agrees | test removed: `length_errors`, `header_and_extent_errors`, `stream_order_reports_the_first_fault` |
| F-28 | §8.1.3.5 ¶5 NOTE 2 "sender's option" to pad | agrees; "limit applies to the value" is policy (the standard sets no limit) | BER refuses more than 8 length octets: `length_non_minimal_long_forms_ber_accepts_der_rejects`, `tlv_non_minimal_length_is_kept_and_der_rejects_it` |
| F-30, F-31 | §8.1.3.6.2; §8.1.5 ¶1-2 "two zero octets" | agrees; "the first EOC at their level" is derived from nesting (the clause does not say how the end is found) | `00 xx` taken as EOC: `eoc_errors`; missing EOC accepted: `eoc_errors`, `truncation_sweep_c_indefinite`. `indefinite_forms_ber` did not fail under either |
| F-35 | §8.1.3.3 ¶1; §8.1.4 ¶1 | agrees (derived: the class `length_overrun` is this crate's) | `length_overrun` read as `truncated`: `header_and_extent_errors`, `stream_order_reports_the_first_fault`, `truncation_sweep_inside_a_definite_parent_is_length_overrun_when_the_parent_lies` |
| Truncation | §8.1.1.1 ¶1 | agrees (derived: a proper prefix lacks a component or contents octets) | definite-length extent check removed: 8 tests, among them the three truncation sweeps and `annex_a_record_parses_and_every_prefix_is_refused`; missing EOC accepted: `truncation_sweep_c_indefinite` |
| Exact slices | §11.6 ¶1 "compared as octet strings" | agrees: the SET OF order is compared on the encodings, so the originals are kept | content slice widened to header and contents: `tlv_slices_are_the_exact_input_octets`, `children_iterate_without_allocation_and_keep_offsets` and 14 more; offset made relative: `children_iterate_without_allocation_and_keep_offsets`, `indefinite_forms_ber`. Making the whole-encoding slice equal the contents made the reader loop forever, not fail (test run killed after 100 s) |
| Writer | §8.1.2.4.2 c); §8.1.3.5 and §10.1 ¶1 minimal length; §8.1.5 ¶3-4 NOTE (EOC as UNIVERSAL 0) | agrees; refusing UNIVERSAL 0 and indefinite primitive is derived | UNIVERSAL 0 allowed, indefinite primitive allowed: `writer_refusals`; tag minimal to 30: `tag_vectors_...`, `tag_encoded_len_boundaries`; length minimal (F-19 break): `length_vectors_ber_and_der`. Short buffer: the check in `write_header` is redundant (removing it failed nothing); the checks in `Tag::encode` and `Length::encode` are covered by `tag_encode_buffer_too_small` and `length_encode_buffer_too_small`, not by `writer_refusals` |

### Rows left `[unread]`

| Row | Why |
|---|---|
| F-16 | Figure 4 of §8.1.2.4.3 and Erratum 1 (09/2021: "Figure 4 contains a typo in the penultimate octet") are images the extracted text does not carry. The behaviour follows from §8.1.2.4.2 a) (bit 8 is one on every subsequent octet but the last), which was read, but the figure itself was not |
| F-17 | A POLICY row. X.680 §8.2 "The number is a non-negative integer" sets no maximum and a search of X.690 found none, so the policy is confirmed as a choice. The other POLICY rows are `[open]`; whether this one follows them is for the coordinator to decide. Under a breakage `tag_errors` and `header_and_extent_errors` fail |
| F-44 | §10 ¶1, with the titles of §10.1 to §10.3 and §11.1 to §11.9, shows no identifier-octet rule beyond BER, so the row agrees with the clause. But the named test `header_and_extent_errors` did not fail when a DER-only tag rule was added (`tlv_vectors_ok_in_both_modes` and `round_trip_writer_to_reader_with_literal_check` did), so no named test is shown to guard it |
| Stream order | The anchor is `corpus/format.md`, not X.690; there is no clause to read |

The `[unread]` rows of the other modules cite ISO 6093, X.660, ISO/IEC 10646 or the ISO register, none of them in the
corpus. The X.690 and X.680 clauses they also cite were read on 2026-11-13 and say what those rows say (X.690 §11.3.2.6
"+0", §8.19.4 NOTE on 39, §8.23.10 ¶1 "ISO/IEC 10646, Annex D", §11.4 escape sequences, X.680 §12.27 and §41.6 defer
to X.660 and 10646), so those rows stay `[unread]` until the external document is read.
