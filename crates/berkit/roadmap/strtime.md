# Roadmap fragment: character strings and time (`strings.rs`, `time.rs`)

Obligation ids are those of `docs/design/strings-time.md`. `[ok]` = the named test exists, passes, and failed under a
deliberate breakage (35 breakages run, each restored from a backup outside the repository and `diff`-verified).
`[unread]` = the anchor could not be read (ISO/IEC 10646, ISO 8601, the ISO register); where a POLICY is implemented
and tested it is named. `[open]` = not done here. All tests are in `crates/berkit/tests/strings_time.rs`.

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [open] | S01 tag is the string's own universal tag | X.690 §8.23.3 | framing's job; `tag_numbers` only checks the table and was not broken |
| [ok] | S02 Numeric/Printable/Visible/IA5 octets are the ISO 646 cells | X.690 §8.23.4; X.680 §43.5-43.8 | `printable_accepts_exactly_the_74_characters`, `numeric_accepts_exactly_the_11_characters`, `visible_is_0x20_to_0x7e`, `ia5_is_0x00_to_0x7f` |
| [open] | S03 values encoded independently | X.690 §8.23.2 | structural (a `Validator` is per value); no test can fail |
| [ok] | S04 constructed accepted in BER | X.690 §8.23.6, §7.3 | `constructed_strings_are_refused_in_der_only` (form flag only; segment framing is the bit/octet module's) |
| [ok] | S05 DER refuses constructed strings (`constructed_string`) | X.690 §10.2 | `constructed_strings_are_refused_in_der_only` |
| [ok] | S06 segments: assemble first, validate the whole | X.690 §8.7.3.1-2, §8.23.3 | `a_character_split_across_two_segments_is_one_character`, `segment_errors_count_over_all_segments` |
| [open] | S07 CER chunking | X.690 §9.2 | CER is out of scope |
| [open] | S08 length forms | X.690 §8.1.3 | framing |
| [ok] | S09 no ESC designation in Numeric/Printable/Visible | X.690 §8.23.5.1, Table 3 | `visible_is_0x20_to_0x7e` (ESC 0x1B refused; IA5 keeps it as a C0 control) |
| [ok] | S10 octet outside the alphabet rejected (POLICY) | X.680 §41 | `printable_boundaries`, `numeric_accepts_exactly_the_11_characters`, `ia5_is_0x00_to_0x7f` |
| [ok] | S11 controls only where a C0 is assumed | X.690 §8.23.5.2, Table 3 | `visible_is_0x20_to_0x7e`, `ia5_is_0x00_to_0x7f` |
| [ok] | S12 announcers/escapes tolerated as opaque octets in the register types (POLICY) | X.690 §8.23.5.4 | `register_types_accept_any_octets`; the four register alphabets themselves are `[unread]` (ISO register not in the corpus) |
| [ok] | S13 UniversalString: 4 octets, no signature | X.690 §8.23.7 | `universal_valid_and_length`, `bmp_and_universal_signature_is_refused_at_the_start_only` |
| [ok] | S14 BMPString: 2 octets, even length, no signature | X.690 §8.23.8 | `bmp_valid`, `bmp_odd_length_is_refused`, `bmp_and_universal_signature_is_refused_at_the_start_only` |
| [ok] | S15 BMPString refuses surrogate cells | X.680 §41.15 | `bmp_surrogates_are_refused`. U+FFFE/U+FFFF refused as POLICY (`bmp_fffe_and_ffff_are_policy_refused`), the notes' inference, not stated |
| [open] | S16 no ISO 2022 escapes in BMP/Universal | X.690 §8.23.9 | POLICY: treated as code points; nothing detects them |
| [unread] | S17 UTF8String octets per Annex D | X.690 §8.23.10 ¶1; ISO/IEC 10646 Annex D | RFC 3629 §4 grammar implemented as POLICY: `utf8_valid_sequences`, `utf8_surrogates_are_refused`, `utf8_above_10ffff_is_refused`, `utf8_truncated_and_stray_continuation` (broken and failing, so `[ok]` against RFC 3629, `[unread]` against 10646) |
| [ok] | S18 UTF-8 smallest number of octets | X.690 §8.23.10 ¶2 | `utf8_overlong_is_refused` |
| [unread] | S19 UniversalString/UTF8String abstract set | X.680 §41.6, §41.16; ISO/IEC 10646 | policy: surrogates and above U+10FFFF refused (`universal_surrogate_and_range_policy`) |
| [open] | S20 CHARACTER STRING | X.690 §8.24 | structured module |
| [unread] | S21 register escape generation (encoder) | X.690 §11.4 | ISO register not in the corpus |
| [open] | S22 CER/DER reading of "BER" | X.690 §11 | no code |
| [ok] | T01 UTCTime/GeneralizedTime form (primitive; constructed BER, refused DER: POLICY, conflict C4) | X.690 §8.25, §10.2 | `constructed_strings_are_refused_in_der_only`; tags are framing's |
| [ok] | T02 UTCTime grammar | X.680 §47.3 | `utctime_grammar_errors`, `utctime_minutes_only_and_offsets`. Offset digit ranges are POLICY, `[unread]` ISO 8601 |
| [ok] | T03 offset sign convention | X.680 §47.3, §46.2 NOTE | `utctime_minutes_only_and_offsets`, `generalized_time_zones` |
| [ok] | T04 GeneralizedTime grammar | X.680 §46.2, §46.3 | `generalized_time_zones`, `generalized_time_fractions`, `generalized_time_range_and_grammar_errors`. Leap second (refused) and bare decimal sign (refused) are POLICY, `[unread]` ISO 8601 |
| [ok] | T05 DER UTCTime: Z, seconds | X.690 §11.8.1-3 | `utctime_standard_invalid_examples`, `utctime_minutes_only_and_offsets` |
| [ok] | T06 DER GeneralizedTime: Z, seconds, `.`, no trailing zero | X.690 §11.7.1-5 | `generalized_time_standard_examples`, `generalized_time_standard_invalid_examples`, `generalized_time_fractions`, `generalized_time_zones` |
| [ok] | T07 the standard's valid and invalid examples, hh 24 refused | X.690 §11.7.5, §11.8.4-5 | the tests above; `19920722132100.3Z` is DER-valid and refused only by `parse_rfc5280_generalized_time` |
| [ok] | T08 century pivot is a parameter | RFC 5280 §4.1.2.5.1 (the standards give none) | `utctime_century_pivot_is_a_parameter` |
| [open] | T09 value equality ignores accuracy | X.680 §46.3 NOTE 3 | no comparison API |
| [open] | T10 canonicalise before comparing | X.680 Annex C | no comparison API |
| [ok] | T11 precision: minute or second (UTCTime); hour, minute, second, fraction (GT) | X.680 §47.2, §46.2 | `utctime_minutes_only_and_offsets`, `generalized_time_zones` |
| [open] | M01 tags 14, 31-34 (`1F 1F` to `1F 22`) | X.680 §38 | framing's job |
| [open] | M02 TIME types are primitive | X.690 §8.26.x.1 | framing's job (`wrong_form`) |
| [ok] | M03 TIME contents: printable ASCII, quotes removed | X.690 §8.26.1.2 | `general_time_contents`. The full TIME grammar (Tables 6, 7) is not implemented (`[open]`); X.680 §12.16 not re-read |
| [ok] | M04 DATE `YYYYMMDD` | X.690 §8.26.2.2 | `date_time_of_day_date_time` |
| [ok] | M05 TIME-OF-DAY `hhmmss`, 24:00:00 allowed | X.690 §8.26.3.2; X.680 §38.4.2 | `date_time_of_day_date_time` |
| [ok] | M06 DATE-TIME `YYYYMMDDhhmmss` | X.690 §8.26.4.2 | `date_time_of_day_date_time` |
| [ok] | M07 DURATION grammar after `P` removal | X.690 §8.26.5.2; X.680 §38.3.6 | `duration_valid`, `duration_grammar_errors` |
| [ok] | M08 DER: comma refused | X.690 §11.9.1 a) | `duration_der_canonical_form`, `general_time_contents` |
| [ok] | M09 DER: `+hh:00` refused (TIME, lexical check) | X.690 §11.9.1 b) | `general_time_contents` |
| [open] | M10 DER: end-point difference equal to start's (intervals) | X.690 §11.9.1 c) | no interval grammar |
| [ok] | M11 DER: zero components removed except the last | X.690 §11.9.1 d) | `duration_der_canonical_form` |
| [open] | M12 canonical notation then encoded | X.690 §11.9.2 | no TIME encoder |
| [ok] | M13 duration integral part, fraction last | X.680 §38.3.6.11-13 | `duration_grammar_errors` |
| [open] | M14 accuracy is part of the value | X.680 §38.3.6.14 | structural (digits kept, nothing normalised); `duration_valid` accepts `29M` and `29M0D` but no test can fail |
| [ok] | M15 DATE/DATE-TIME year from 1582 | X.680 Table 6 (taken from the notes, not re-read) | `date_time_of_day_date_time` (floor only; the ceiling is the four-digit width) |

## Missing reference material

- ISO/IEC 10646 (Annex D, signatures, the "64K-2" cells, the UCS-4 range): UTF-8 validity, BMP U+FFFE/U+FFFF, UniversalString range.
- ISO 8601 (4.1.2.2, 4.2.2.2-4, 4.2.5.2): leap second, offset digit ranges, decimal sign without digit, days per month.
- ISO International Register of Coded Character Sets, ISO/IEC 2022 and 2375: Teletex, Videotex, Graphic, General alphabets.
- X.680 Tables 6 and 7 (re-read) and §12.16 (`simplestring`): the full TIME grammar and the year range of `Year=Basic`.
