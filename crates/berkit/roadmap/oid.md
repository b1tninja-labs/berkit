# oid roadmap

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | OBJECT IDENTIFIER contents: concatenated base-128 subidentifiers, bit 8 set on all but the last octet | X.690 §8.19.2 ¶1-¶5 | `common_values`, `uuid_arc_under_2_25_is_128_bits` |
| [ok] | First subidentifier is X*40+Y; split at 40 and 80; second arc of any size under arc 2 (`2.100.3` = `81 34 03`, `2.999.3` = `88 37 03`) | X.690 §8.19.4, §8.19.5 | `first_two_arcs_split_at_40_and_80`, `the_x690_example_2_999_3_is_88_37_03`, `the_2008_example_2_100_3_is_81_34_03` |
| [ok] | Arcs of 2^32, 2^64-1, 2^64, 2^128-1 and 2^128 never truncate; `to_u64`/`to_u128` give `None` on overflow; `be_bytes` exact | X.690 §8.19.2 (no limit stated) | `arcs_around_two_pow_32_and_64_never_truncate`, `largest_second_arc_under_2_needs_the_carry_past_128_bits`, `be_bytes_gives_exact_values_including_the_bias` |
| [ok] | Subidentifier with leading octet `80` refused (`non_minimal_subidentifier`), BER and DER | X.690 §8.19.2 ¶6 | `leading_80_is_refused_in_ber_and_der` |
| [ok] | Subidentifier that never ends refused. POLICY: `bad_contents`, not `truncated` (the declared contents are complete) | X.690 §8.19.2 ¶3 | `a_subidentifier_that_never_ends_is_bad_contents` |
| [ok] | Empty contents refused (`bad_contents`). POLICY, derived | X.690 §8.19.3, X.680 §32.11 | `empty_contents_are_refused` |
| [ok] | Single subidentifier is two arcs; Annex B values `2.1.1`, `2.1.2.0`, `2.1.2.1` (bytes derived by hand) | X.690 §8.19.4, Annex B | `single_subidentifier_is_two_arcs`, `annex_b_values_contents` |
| [ok] | Minimal encoder from `u64`, `u128` and big-endian arcs; short buffer reported | X.690 §8.19.2 ¶6 | `common_values`, `big_endian_arcs_ignore_leading_zeros_and_empty_is_zero`, `encoder_reports_a_short_buffer_at_every_length` |
| [ok] | RELATIVE-OID: same subidentifier rules, no packing of the first two arcs, empty refused (derived) | X.690 §8.20.2-§8.20.5 | `relative_oid_example_8571_3_2`, `relative_oid_has_no_first_two_arc_packing`, `relative_oid_refusals` |
| [ok] | OID-IRI contents: UTF-8 of `/arc/arc`, leading slash, no empty label, integer label without leading zero, no ill-formed UTF-8 | X.690 §8.21.2, X.680 §34.3, §12.26 | `oid_iri_contents`, `iri_refusals` |
| [ok] | RELATIVE-OID-IRI contents: as OID-IRI with no leading slash. POLICY: the shortest-form check applies though §8.22.2 does not repeat it | X.690 §8.22.2, X.680 §35.3 | `relative_oid_iri_contents`, `iri_refusals` |
| [unread] | First arc is 0, 1 or 2 and the second is at most 39 under 0 and 1 (encoder refuses otherwise; decoder accepts any value) | X.660 (not in the corpus); NOTE in X.690 §8.19.4 | `encoder_refusals` |
| [unread] | Character rules of non-integer Unicode labels in an IRI (not checked) | X.680 §12.27 ¶1 cites X.660 §7.5 | none |
| [open] | ASCII white-space inside an IRI is refused: derived from "no white-space", not stated | X.690 §8.21.2 ¶1, X.680 §12.27 | `iri_refusals` |
| [open] | Decimal or dotted display of arcs (needs a bignum without allocation); not provided | none | none |
| [open] | Identifier octets `06`, `0D`, `1F 23`, `1F 24` and the primitive-form (`wrong_form`) check belong to the framing module | X.690 §8.19.1, §8.20.1, §8.21.1, §8.22.1 | none |
