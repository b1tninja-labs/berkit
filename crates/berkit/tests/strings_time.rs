//! Integration tests for `berkit::strings` and `berkit::time` over the public API.
//!
//! Provenance labels: `from X.690 <place>` / `from X.680 <place>` is a value printed in the standard;
//! `constructed by hand` is built here from a clause. Nothing comes from a real certificate.

use berkit::strings::{self, StringType};
use berkit::time::{self, Zone};
use berkit::{ErrorClass, Mode};

const BAD: ErrorClass = ErrorClass::BadContents;
const TF: ErrorClass = ErrorClass::TimeFormat;

fn err_s(ty: StringType, b: &[u8]) -> (ErrorClass, usize) {
    let e = strings::validate(ty, b).unwrap_err();
    (e.class, e.offset)
}

fn ok_s(ty: StringType, b: &[u8]) {
    assert_eq!(strings::validate(ty, b), Ok(()), "{b:02X?}");
}

// ---------------------------------------------------------------- alphabets

/// The accepted set of a one-octet type, over every octet.
fn accepted(ty: StringType) -> Vec<u8> {
    (0..=255u8)
        .filter(|&b| strings::validate(ty, &[b]).is_ok())
        .collect()
}

#[test]
fn printable_accepts_exactly_the_74_characters() {
    // from X.680 §41.4 Table 10 and §43.6 ("precisely 74"); the hex mapping is the notes' section 2.2.
    let mut want: Vec<u8> = Vec::new();
    want.extend(b'A'..=b'Z');
    want.extend(b'a'..=b'z');
    want.extend(b'0'..=b'9');
    want.extend(b" '()+,-./:=?");
    want.sort_unstable();
    assert_eq!(want.len(), 74);
    assert_eq!(accepted(StringType::Printable), want);
}

#[test]
fn printable_boundaries() {
    // constructed by hand from X.680 Table 10; notes P02, P03, P08, P09, P10.
    for b in [
        0x20, 0x27, 0x28, 0x29, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x3A, 0x3D, 0x3F, 0x41, 0x5A, 0x61,
        0x7A, 0x30, 0x39,
    ] {
        ok_s(StringType::Printable, &[b]);
    }
    for b in [
        0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x2A, 0x3B, 0x3C, 0x3E, 0x40, 0x5B, 0x5F, 0x60, 0x7B,
        0x7F, 0x1F, 0x00, 0x80, 0xFF,
    ] {
        assert_eq!(err_s(StringType::Printable, &[b]), (BAD, 0), "{b:#x}");
    }
}

#[test]
fn printable_apostrophe_yes_at_sign_star_ampersand_no() {
    // constructed by hand; notes P01, P04 to P06.
    ok_s(StringType::Printable, b"ABCXYZabcxyz0189 '()+,-./:=?");
    assert_eq!(err_s(StringType::Printable, b"A@B"), (BAD, 1));
    assert_eq!(err_s(StringType::Printable, b"a*b"), (BAD, 1));
    assert_eq!(err_s(StringType::Printable, b"a&b"), (BAD, 1));
    assert_eq!(err_s(StringType::Printable, b"ab_"), (BAD, 2));
    ok_s(StringType::Printable, b"");
}

#[test]
fn numeric_accepts_exactly_the_11_characters() {
    // from X.680 §41.2 Table 9, §43.5 ("precisely 11").
    let mut want = vec![b' '];
    want.extend(b'0'..=b'9');
    assert_eq!(accepted(StringType::Numeric), want);
    ok_s(StringType::Numeric, b"0123456789 ");
    ok_s(StringType::Numeric, b"");
    assert_eq!(err_s(StringType::Numeric, b"12a"), (BAD, 2));
    for b in [0x2D, 0x2F, 0x3A, 0x1F, 0x80] {
        assert_eq!(err_s(StringType::Numeric, &[b]), (BAD, 0));
    }
}

#[test]
fn visible_is_0x20_to_0x7e() {
    // from X.680 §43.7 ("cells 2/0-7/14", 95 characters).
    assert_eq!(
        accepted(StringType::Visible),
        (0x20..=0x7Eu8).collect::<Vec<_>>()
    );
    for b in [0x7F, 0x1F, 0x80] {
        assert_eq!(err_s(StringType::Visible, &[b]), (BAD, 0));
    }
}

#[test]
fn ia5_is_0x00_to_0x7f() {
    // from X.680 Table 8 and §43.8 (128 characters, DELETE included).
    assert_eq!(
        accepted(StringType::Ia5),
        (0x00..=0x7Fu8).collect::<Vec<_>>()
    );
    ok_s(StringType::Ia5, &[0x00]);
    ok_s(StringType::Ia5, &[0x7F]);
    assert_eq!(err_s(StringType::Ia5, &[0x41, 0x80]), (BAD, 1));
    assert_eq!(err_s(StringType::Ia5, &[0xFF]), (BAD, 0));
}

#[test]
fn register_types_accept_any_octets() {
    // POLICY: the ISO register is not in the corpus (`[unread]`), so no alphabet is checked.
    let all: Vec<u8> = (0..=255).collect();
    for ty in [
        StringType::Teletex,
        StringType::Videotex,
        StringType::Graphic,
        StringType::General,
    ] {
        ok_s(ty, &all);
    }
}

#[test]
fn tag_numbers() {
    // from X.680 Table 1.
    let t = |ty: StringType| ty.tag_number();
    assert_eq!(t(StringType::Utf8), 12);
    assert_eq!(t(StringType::Numeric), 18);
    assert_eq!(t(StringType::Printable), 19);
    assert_eq!(t(StringType::Ia5), 22);
    assert_eq!(t(StringType::Visible), 26);
    assert_eq!(t(StringType::Universal), 28);
    assert_eq!(t(StringType::Bmp), 30);
}

// ---------------------------------------------------------------- UTF-8

#[test]
fn utf8_valid_sequences() {
    // constructed by hand from X.690 §8.23.10; notes U01 to U03.
    for v in [
        &b"abc"[..],
        &[0xC3, 0xA9],
        &[0xE2, 0x82, 0xAC],
        &[0xF0, 0x9F, 0x98, 0x80],
        &[0xC2, 0x80],
        &[0xDF, 0xBF],
        &[0xE0, 0xA0, 0x80],
        &[0xED, 0x9F, 0xBF],
        &[0xEE, 0x80, 0x80],
        &[0xF4, 0x8F, 0xBF, 0xBF], // U+10FFFF
        &[],
    ] {
        ok_s(StringType::Utf8, v);
    }
}

#[test]
fn utf8_overlong_is_refused() {
    // X.690 §8.23.10 ¶2 (smallest number of octets); notes U04, U05.
    assert_eq!(err_s(StringType::Utf8, &[0xC0, 0xAF]), (BAD, 0));
    assert_eq!(err_s(StringType::Utf8, &[0xC1, 0xBF]), (BAD, 0));
    assert_eq!(err_s(StringType::Utf8, &[0xE0, 0x80, 0xAF]), (BAD, 1));
    assert_eq!(err_s(StringType::Utf8, &[0xF0, 0x80, 0x80, 0xAF]), (BAD, 1));
}

#[test]
fn utf8_surrogates_are_refused() {
    // POLICY (RFC 3629 §4, ISO/IEC 10646 `[unread]`); notes U06.
    assert_eq!(err_s(StringType::Utf8, &[0xED, 0xA0, 0x80]), (BAD, 1));
    assert_eq!(err_s(StringType::Utf8, &[0xED, 0xBF, 0xBF]), (BAD, 1));
}

#[test]
fn utf8_above_10ffff_is_refused() {
    // POLICY (RFC 3629 §4); notes U09 is `F4 90 80 80`.
    assert_eq!(err_s(StringType::Utf8, &[0xF4, 0x90, 0x80, 0x80]), (BAD, 1));
    assert_eq!(err_s(StringType::Utf8, &[0xF5, 0x80, 0x80, 0x80]), (BAD, 0));
    assert_eq!(err_s(StringType::Utf8, &[0xFF]), (BAD, 0));
}

#[test]
fn utf8_truncated_and_stray_continuation() {
    // constructed by hand; notes U07, U08.
    assert_eq!(err_s(StringType::Utf8, &[0x41, 0xE2, 0x82]), (BAD, 1));
    assert_eq!(err_s(StringType::Utf8, &[0xF0, 0x9F, 0x98]), (BAD, 0));
    assert_eq!(err_s(StringType::Utf8, &[0xC3]), (BAD, 0));
    assert_eq!(err_s(StringType::Utf8, &[0x80]), (BAD, 0));
    assert_eq!(err_s(StringType::Utf8, &[0xBF, 0x41]), (BAD, 0));
    // a lead followed by a non-continuation octet
    assert_eq!(err_s(StringType::Utf8, &[0xC3, 0x41]), (BAD, 1));
    assert_eq!(err_s(StringType::Utf8, &[0xE2, 0x82, 0x41]), (BAD, 2));
}

// ---------------------------------------------------------------- BMP, Universal

#[test]
fn bmp_valid() {
    // constructed by hand from X.690 §8.23.8 ¶1; notes B01, B02.
    ok_s(StringType::Bmp, &[0x00, 0x41]);
    ok_s(StringType::Bmp, &[0x00, 0x48, 0x00, 0x69]);
    ok_s(StringType::Bmp, &[0xD7, 0xFF]);
    ok_s(StringType::Bmp, &[0xE0, 0x00]);
    ok_s(StringType::Bmp, &[0xFF, 0xFD]);
    ok_s(StringType::Bmp, &[]);
}

#[test]
fn bmp_odd_length_is_refused() {
    // notes B03: a character cut short is reported at its first octet.
    assert_eq!(err_s(StringType::Bmp, &[0x00, 0x41, 0x00]), (BAD, 2));
    assert_eq!(err_s(StringType::Bmp, &[0x41]), (BAD, 0));
}

#[test]
fn bmp_surrogates_are_refused() {
    // X.680 §41.15; notes B04, B05.
    assert_eq!(err_s(StringType::Bmp, &[0xD8, 0x00]), (BAD, 0));
    assert_eq!(err_s(StringType::Bmp, &[0x00, 0x41, 0xDF, 0xFF]), (BAD, 2));
    assert_eq!(
        err_s(StringType::Bmp, &[0xD8, 0x3D, 0xDE, 0x00]),
        (BAD, 0),
        "a UTF-16 pair is not the 2-octet BMP form"
    );
}

#[test]
fn bmp_fffe_and_ffff_are_policy_refused() {
    // POLICY: the notes' inference from "64K-2" (X.680 §41.15), not stated by the standard; notes B08.
    assert_eq!(err_s(StringType::Bmp, &[0xFF, 0xFE]), (BAD, 0));
    assert_eq!(err_s(StringType::Bmp, &[0xFF, 0xFF]), (BAD, 0));
}

#[test]
fn bmp_and_universal_signature_is_refused_at_the_start_only() {
    // X.690 §8.23.7 ¶2, §8.23.8 ¶2 (signatures shall not be used); notes B06.
    assert_eq!(err_s(StringType::Bmp, &[0xFE, 0xFF, 0x00, 0x41]), (BAD, 0));
    ok_s(StringType::Bmp, &[0x00, 0x41, 0xFE, 0xFF]);
    assert_eq!(err_s(StringType::Universal, &[0, 0, 0xFE, 0xFF]), (BAD, 0));
    ok_s(StringType::Universal, &[0, 0, 0, 0x41, 0, 0, 0xFE, 0xFF]);
}

#[test]
fn universal_valid_and_length() {
    // constructed by hand from X.690 §8.23.7 ¶1; notes C01, C02, C04.
    ok_s(StringType::Universal, &[0, 0, 0, 0x41]);
    ok_s(StringType::Universal, &[0, 1, 0xF6, 0x00]);
    ok_s(StringType::Universal, &[0, 0x10, 0xFF, 0xFF]);
    assert_eq!(err_s(StringType::Universal, &[0, 0, 0x41]), (BAD, 0));
    assert_eq!(
        err_s(StringType::Universal, &[0, 0, 0, 0x41, 0, 0]),
        (BAD, 4)
    );
}

#[test]
fn universal_surrogate_and_range_policy() {
    // POLICY (ISO/IEC 10646 `[unread]`); notes C03.
    assert_eq!(err_s(StringType::Universal, &[0, 0, 0xD8, 0x00]), (BAD, 0));
    assert_eq!(err_s(StringType::Universal, &[0, 0x11, 0, 0]), (BAD, 0));
    assert_eq!(err_s(StringType::Universal, &[0x80, 0, 0, 0]), (BAD, 0));
}

// ---------------------------------------------------------------- segments

#[test]
fn a_character_split_across_two_segments_is_one_character() {
    // constructed by hand from X.690 §8.7.3.1 ¶2; notes U10, B07: segment boundary inside the character.
    let segs = |parts: &[&'static [u8]]| {
        strings::validate_segments(StringType::Utf8, parts.iter().copied())
    };
    assert_eq!(segs(&[&[0xE2], &[0x82, 0xAC]]), Ok(()));
    assert_eq!(segs(&[&[0xE2, 0x82], &[], &[0xAC]]), Ok(()));
    assert_eq!(
        strings::validate_segments(StringType::Bmp, [&[0x00][..], &[0x41]]),
        Ok(())
    );
    assert_eq!(
        strings::validate_segments(StringType::Universal, [&[0, 0][..], &[0][..], &[0x41][..]]),
        Ok(())
    );
    // each half alone is invalid, which a per-segment check would have reported
    assert!(strings::validate(StringType::Utf8, &[0xE2]).is_err());
    assert!(strings::validate(StringType::Bmp, &[0x00]).is_err());
}

#[test]
fn segment_errors_count_over_all_segments() {
    // constructed by hand: offset 3 is the second octet of the second segment.
    let e =
        strings::validate_segments(StringType::Printable, [&b"ab"[..], &b"c@"[..]]).unwrap_err();
    assert_eq!((e.class, e.offset), (BAD, 3));
    // a character left open at the end of the last segment
    let e = strings::validate_segments(StringType::Utf8, [&b"ab"[..], &[0xE2][..], &[0x82][..]])
        .unwrap_err();
    assert_eq!((e.class, e.offset), (BAD, 2));
    // no segments is the empty string
    assert_eq!(
        strings::validate_segments(StringType::Utf8, core::iter::empty::<&[u8]>()),
        Ok(())
    );
}

#[test]
fn constructed_strings_are_refused_in_der_only() {
    // X.690 §10.2 ¶1; notes V02d.
    assert_eq!(strings::check_form(Mode::Ber, true), Ok(()));
    assert_eq!(strings::check_form(Mode::Ber, false), Ok(()));
    assert_eq!(strings::check_form(Mode::Der, false), Ok(()));
    assert_eq!(
        strings::check_form(Mode::Der, true).unwrap_err().class,
        ErrorClass::ConstructedString
    );
    // POLICY: the same for the two useful time types.
    assert_eq!(
        time::check_form(Mode::Der, true).unwrap_err().class,
        ErrorClass::ConstructedString
    );
    assert_eq!(time::check_form(Mode::Ber, true), Ok(()));
}

#[test]
fn per_type_wrappers_agree_with_validate() {
    assert!(strings::validate_numeric(b"1 2").is_ok());
    assert!(strings::validate_printable(b"a@").is_err());
    assert!(strings::validate_visible(b"~").is_ok());
    assert!(strings::validate_ia5(&[0x7F]).is_ok());
    assert!(strings::validate_utf8(&[0xC0, 0xAF]).is_err());
    assert!(strings::validate_bmp(&[0, 0x41]).is_ok());
    assert!(strings::validate_universal(&[0, 0, 0, 0x41]).is_ok());
    assert!(strings::is_numeric_octet(b'5') && !strings::is_numeric_octet(b'a'));
    assert!(strings::is_printable_octet(b'\'') && !strings::is_printable_octet(b'@'));
    assert!(strings::is_visible_octet(b'~') && !strings::is_visible_octet(0x7F));
    assert!(strings::is_ia5_octet(0x7F) && !strings::is_ia5_octet(0x80));
}

// ---------------------------------------------------------------- UTCTime

fn utc(s: &str, mode: Mode, pivot: u8) -> Result<time::Fields<'_>, (ErrorClass, usize)> {
    time::parse_utc_time(s.as_bytes(), mode, pivot).map_err(|e| (e.class, e.offset))
}

fn gt(s: &str, mode: Mode) -> Result<time::Fields<'_>, (ErrorClass, usize)> {
    time::parse_generalized_time(s.as_bytes(), mode).map_err(|e| (e.class, e.offset))
}

#[test]
fn utctime_standard_examples_are_valid_in_both_modes() {
    // from X.690 §11.8.4.
    for s in ["920521000000Z", "920622123421Z", "920722132100Z"] {
        for m in [Mode::Ber, Mode::Der] {
            let f = utc(s, m, 50).unwrap();
            assert_eq!(f.zone, Zone::Utc);
            assert!(f.second.is_some());
        }
    }
    let f = utc("920622123421Z", Mode::Der, 50).unwrap();
    assert_eq!(
        (f.year, f.month, f.day, f.hour, f.minute, f.second),
        (1992, 6, 22, 12, Some(34), Some(21))
    );
    assert_eq!(f.fraction, b"");
}

#[test]
fn utctime_standard_invalid_examples() {
    // from X.690 §11.8.5: 24:00 is wrong in both modes, missing seconds only in DER.
    assert_eq!(utc("920520240000Z", Mode::Ber, 50), Err((BAD, 6)));
    assert_eq!(utc("920520240000Z", Mode::Der, 50), Err((BAD, 6)));
    assert!(utc("9207221321Z", Mode::Ber, 50).is_ok());
    assert_eq!(utc("9207221321Z", Mode::Der, 50), Err((TF, 10)));
}

#[test]
fn utctime_minutes_only_and_offsets() {
    // from X.680 §47.3 Example 1 and 2; notes UT05 to UT08.
    let f = utc("8201020700-0500", Mode::Ber, 50).unwrap();
    assert_eq!(f.second, None);
    assert_eq!(
        f.zone,
        Zone::Offset {
            negative: true,
            hour: 5,
            minute: 0,
            minute_present: true
        }
    );
    assert_eq!(f.zone.minutes_east(), Some(-300));
    assert_eq!(utc("8201020700-0500", Mode::Der, 50), Err((TF, 10)));
    assert_eq!(utc("0101021200Z", Mode::Ber, 50).unwrap().year, 2001);
    assert_eq!(utc("0101021200Z", Mode::Der, 50), Err((TF, 10)));
    let f = utc("920622123421+0100", Mode::Ber, 50).unwrap();
    assert_eq!(f.zone.minutes_east(), Some(60));
    assert_eq!(utc("920622123421+0100", Mode::Der, 50), Err((TF, 12)));
    assert_eq!(Zone::Utc.minutes_east(), Some(0));
    assert_eq!(Zone::Local.minutes_east(), None);
}

#[test]
fn utctime_grammar_errors() {
    // notes UT09 to UT11, constructed by hand from X.680 §47.3.
    assert_eq!(
        utc("920622123421", Mode::Ber, 50),
        Err((BAD, 12)),
        "zone is mandatory"
    );
    assert_eq!(utc("920622123461Z", Mode::Ber, 50), Err((BAD, 10)), "ss 61");
    assert_eq!(utc("920622123460Z", Mode::Ber, 50), Err((BAD, 10)), "ss 60");
    assert_eq!(
        utc("921322123421Z", Mode::Ber, 50),
        Err((BAD, 2)),
        "month 13"
    );
    assert_eq!(
        utc("920022123421Z", Mode::Ber, 50),
        Err((BAD, 2)),
        "month 00"
    );
    assert_eq!(utc("920600123421Z", Mode::Ber, 50), Err((BAD, 4)), "day 00");
    assert_eq!(utc("920632123421Z", Mode::Ber, 50), Err((BAD, 4)), "day 32");
    assert_eq!(utc("920622126021Z", Mode::Ber, 50), Err((BAD, 8)), "mm 60");
    assert_eq!(
        utc("920622123421Zx", Mode::Ber, 50),
        Err((BAD, 13)),
        "trailing"
    );
    assert_eq!(
        utc("92062212342", Mode::Ber, 50),
        Err((BAD, 11)),
        "odd digits"
    );
    assert_eq!(utc("9206221234Z1", Mode::Ber, 50), Err((BAD, 11)));
    assert_eq!(
        utc("92O622123421Z", Mode::Ber, 50),
        Err((BAD, 2)),
        "letter O"
    );
    assert_eq!(utc("", Mode::Ber, 50), Err((BAD, 0)));
    assert_eq!(
        utc("920622123421+01", Mode::Ber, 50),
        Err((BAD, 15)),
        "offset needs hhmm"
    );
    assert_eq!(
        utc("920622123421+2400", Mode::Ber, 50),
        Err((BAD, 13)),
        "POLICY offset hh 24"
    );
    assert_eq!(
        utc("920622123421+0160", Mode::Ber, 50),
        Err((BAD, 15)),
        "POLICY offset mm 60"
    );
    assert!(
        utc("920631123421Z", Mode::Ber, 50).is_ok(),
        "day 31 of a 30-day month is only a calendar matter"
    );
    assert!(
        !utc("920631123421Z", Mode::Ber, 50)
            .unwrap()
            .is_calendar_date()
    );
}

#[test]
fn utctime_century_pivot_is_a_parameter() {
    // notes UT12 to UT15 (constructed by hand): the standard gives no rule, so the pivot decides.
    let y = |s, p| utc(s, Mode::Der, p).unwrap().year;
    assert_eq!(y("491231235959Z", 50), 2049);
    assert_eq!(y("500101000000Z", 50), 1950);
    assert_eq!(y("000101000000Z", 50), 2000);
    assert_eq!(y("991231235959Z", 50), 1999);
    // another pivot moves the line
    assert_eq!(y("491231235959Z", 40), 1949);
    assert_eq!(y("391231235959Z", 40), 2039);
    assert_eq!(y("400101000000Z", 40), 1940);
    assert_eq!(y("991231235959Z", 100), 2099);
    assert_eq!(y("000101000000Z", 0), 1900);
    assert_eq!(time::RFC5280_PIVOT, 50);
    // RFC 5280 §4.1.2.5.1
    assert_eq!(
        time::parse_rfc5280_utc_time(b"491231235959Z").unwrap().year,
        2049
    );
    assert_eq!(
        time::parse_rfc5280_utc_time(b"500101000000Z").unwrap().year,
        1950
    );
    assert!(time::parse_rfc5280_utc_time(b"5001010000Z").is_err());
}

// ---------------------------------------------------------------- GeneralizedTime

#[test]
fn generalized_time_standard_examples() {
    // from X.690 §11.7.5: valid in DER.
    for s in ["19920521000000Z", "19920622123421Z", "19920722132100.3Z"] {
        for m in [Mode::Ber, Mode::Der] {
            assert!(gt(s, m).is_ok(), "{s}");
        }
    }
    // The standard's own example is DER-valid; the RFC 5280 profile is narrower and refuses it.
    let f = gt("19920722132100.3Z", Mode::Der).unwrap();
    assert_eq!(f.fraction, b"3");
    assert!(!f.comma);
    assert_eq!(f.zone, Zone::Utc);
    assert_eq!((f.year, f.month, f.day, f.hour), (1992, 7, 22, 13));
    assert_eq!((f.minute, f.second), (Some(21), Some(0)));
    assert!(time::parse_generalized_time(b"19920722132100.3Z", Mode::Der).is_ok());
    assert_eq!(
        time::parse_rfc5280_generalized_time(b"19920722132100.3Z")
            .unwrap_err()
            .class,
        TF
    );
    assert!(time::parse_rfc5280_generalized_time(b"19920622123421Z").is_ok());
}

#[test]
fn generalized_time_standard_invalid_examples() {
    // from X.690 §11.7.5.
    assert_eq!(gt("19920520240000Z", Mode::Ber), Err((BAD, 8)));
    assert_eq!(gt("19920520240000Z", Mode::Der), Err((BAD, 8)));
    assert!(gt("19920622123421.0Z", Mode::Ber).is_ok());
    assert_eq!(gt("19920622123421.0Z", Mode::Der), Err((TF, 15)));
    assert!(gt("19920722132100.30Z", Mode::Ber).is_ok());
    assert_eq!(gt("19920722132100.30Z", Mode::Der), Err((TF, 16)));
}

#[test]
fn generalized_time_fractions() {
    // notes GT13, GT14; X.690 §11.7.3.
    assert_eq!(gt("19851106210627.500Z", Mode::Der), Err((TF, 17)));
    assert_eq!(gt("19851106210627.5Z", Mode::Der).unwrap().fraction, b"5");
    assert_eq!(
        gt("19851106210627.500Z", Mode::Ber).unwrap().fraction,
        b"500"
    );
    // any number of digits
    assert_eq!(
        gt("19851106210627.123456789012Z", Mode::Der)
            .unwrap()
            .fraction,
        b"123456789012"
    );
    // comma: BER allows, DER refuses (X.690 §11.7.4); notes GT12
    let f = gt("19851106210627,3Z", Mode::Ber).unwrap();
    assert!(f.comma);
    assert_eq!(f.fraction, b"3");
    assert_eq!(gt("19851106210627,3Z", Mode::Der), Err((TF, 14)));
    // POLICY: a decimal sign needs a digit; notes GT20
    assert_eq!(gt("19851106210627.Z", Mode::Ber), Err((BAD, 15)));
    assert_eq!(gt("19851106210627.", Mode::Ber), Err((BAD, 15)));
}

#[test]
fn generalized_time_zones() {
    // from X.680 §46.3 examples, cases a) to e); notes GT07 to GT11.
    let za = gt("19851106210627.3", Mode::Ber).unwrap();
    assert_eq!(za.zone, Zone::Local);
    assert_eq!(gt("19851106210627.3", Mode::Der), Err((TF, 16)));
    assert_eq!(gt("19851106210627.3Z", Mode::Der).unwrap().zone, Zone::Utc);
    let zc = gt("19851106210627.3-0500", Mode::Ber).unwrap();
    assert_eq!(zc.zone.minutes_east(), Some(-300));
    assert_eq!(gt("19851106210627.3-0500", Mode::Der), Err((TF, 16)));
    let zd = gt("198511062106.456", Mode::Ber).unwrap();
    assert_eq!(
        (zd.minute, zd.second, zd.fraction),
        (Some(6), None, &b"456"[..])
    );
    assert_eq!(gt("198511062106.456", Mode::Der), Err((TF, 12)));
    let ze = gt("1985110621.14159", Mode::Ber).unwrap();
    assert_eq!(
        (ze.minute, ze.second, ze.fraction),
        (None, None, &b"14159"[..])
    );
    assert_eq!(gt("1985110621.14159", Mode::Der), Err((TF, 10)));
    // Z versus an offset of zero are different encodings; DER wants Z (notes GT17, GT15, GT16).
    assert_eq!(
        gt("19851106210627+0000", Mode::Ber)
            .unwrap()
            .zone
            .minutes_east(),
        Some(0)
    );
    assert_eq!(gt("19851106210627+0000", Mode::Der), Err((TF, 14)));
    assert_eq!(gt("1985110621Z", Mode::Ber).unwrap().minute, None);
    assert_eq!(gt("1985110621Z", Mode::Der), Err((TF, 10)));
    let zh = gt("19851106210627.3+05", Mode::Ber).unwrap();
    assert_eq!(
        zh.zone,
        Zone::Offset {
            negative: false,
            hour: 5,
            minute: 0,
            minute_present: false
        }
    );
    assert_eq!(
        gt("19851106210627.3+0530", Mode::Ber)
            .unwrap()
            .zone
            .minutes_east(),
        Some(330)
    );
}

#[test]
fn generalized_time_range_and_grammar_errors() {
    // constructed by hand from X.680 §46.2, §46.3.
    assert!(gt("20240229000000Z", Mode::Der).is_ok());
    assert!(gt("20240229000000Z", Mode::Der).unwrap().is_calendar_date());
    assert!(!gt("20230229000000Z", Mode::Der).unwrap().is_calendar_date());
    assert!(!gt("19000229000000Z", Mode::Der).unwrap().is_calendar_date());
    assert!(gt("20000229000000Z", Mode::Der).unwrap().is_calendar_date());
    // POLICY: no leap second (notes GT18); X.680 §46 states no range
    assert_eq!(gt("19851106215960Z", Mode::Ber), Err((BAD, 12)));
    assert_eq!(gt("19851106216000Z", Mode::Ber), Err((BAD, 10)));
    assert_eq!(gt("19851306210627Z", Mode::Ber), Err((BAD, 4)));
    assert_eq!(gt("19851106240000", Mode::Ber), Err((BAD, 8)));
    assert_eq!(gt("1985110621062", Mode::Ber), Err((BAD, 13)), "odd digits");
    assert_eq!(
        gt("19851106", Mode::Ber),
        Err((BAD, 8)),
        "time of day is mandatory"
    );
    assert_eq!(gt("1985110621Zz", Mode::Ber), Err((BAD, 11)));
    assert_eq!(gt("19851106210627.3+0", Mode::Ber), Err((BAD, 18)));
    assert_eq!(gt("19851106210627Q", Mode::Ber), Err((BAD, 14)));
    assert_eq!(gt("19851106210627.3.4Z", Mode::Ber), Err((BAD, 16)));
    assert_eq!(gt("", Mode::Ber), Err((BAD, 0)));
    // a fraction needs the unit before it: no fraction of a minute after seconds
    assert_eq!(gt("1985110621.5.5", Mode::Ber), Err((BAD, 12)));
}

// ---------------------------------------------------------------- TIME types

#[test]
fn date_time_of_day_date_time() {
    // constructed by hand from X.690 §8.26.2.2, §8.26.3.2, §8.26.4.2 (hyphens, colons and T removed); notes TM01 to TM03.
    let d = time::parse_date(b"20210215").unwrap();
    assert_eq!((d.year, d.month, d.day), (2021, 2, 15));
    let t = time::parse_time_of_day(b"123045").unwrap();
    assert_eq!((t.hour, t.minute, t.second), (12, 30, 45));
    let dt = time::parse_date_time(b"20210215123045").unwrap();
    assert_eq!((dt.date.year, dt.time.second), (2021, 45));
    // X.680 §38.4.2 NOTE: 24:00:00 is allowed, nothing later
    assert!(time::parse_time_of_day(b"240000").is_ok());
    assert!(time::parse_date_time(b"20210215240000").is_ok());
    assert_eq!(time::parse_time_of_day(b"240001").unwrap_err().offset, 0);
    assert_eq!(time::parse_time_of_day(b"250000").unwrap_err().offset, 0);
    assert_eq!(time::parse_time_of_day(b"126000").unwrap_err().offset, 2);
    assert_eq!(time::parse_time_of_day(b"1230").unwrap_err().class, BAD);
    assert_eq!(time::parse_time_of_day(b"12304500").unwrap_err().offset, 6);
    // the hyphenated notation is not the contents
    assert_eq!(time::parse_date(b"2021-02-15").unwrap_err().offset, 4);
    assert_eq!(time::parse_date(b"20211315").unwrap_err().offset, 4);
    assert_eq!(time::parse_date(b"20210232").unwrap_err().offset, 6);
    assert_eq!(time::parse_date(b"15810101").unwrap_err().offset, 0);
    assert_eq!(
        time::parse_date(b"202102150").unwrap_err().offset,
        8,
        "trailing octet"
    );
    assert!(time::parse_date(b"15820101").is_ok());
    assert!(time::parse_date(b"99991231").is_ok());
    assert_eq!(
        time::parse_date_time(b"2021021512304").unwrap_err().class,
        BAD
    );
    assert_eq!(
        time::parse_date_time(b"202102151230450")
            .unwrap_err()
            .offset,
        14
    );
}

#[test]
fn duration_valid() {
    // notes TM04, TM11, TM13; X.680 §38.3.6.14 (P removed per X.690 §8.26.5.2).
    for s in [
        "1Y2M3DT4H5M6S",
        "29M0D",
        "29M",
        "29MT0S",
        "29MT0.00H",
        "29MT0.5S",
        "5W",
        "0D",
        "T5H",
        "T0.5M",
        "1.5Y",
        "10Y",
    ] {
        for m in [Mode::Ber, Mode::Der] {
            assert_eq!(time::validate_duration(s.as_bytes(), m), Ok(()), "{s}");
        }
    }
}

#[test]
fn duration_grammar_errors() {
    // X.680 §38.3.6.1 to §38.3.6.13.
    let e = |s: &str| {
        let e = time::validate_duration(s.as_bytes(), Mode::Ber).unwrap_err();
        (e.class, e.offset)
    };
    assert_eq!(e(""), (BAD, 0));
    assert_eq!(e("P1Y"), (BAD, 0), "the P is removed from the contents");
    assert_eq!(e("1"), (BAD, 1), "no designator");
    assert_eq!(e("Y"), (BAD, 0));
    assert_eq!(e("T"), (BAD, 1), "T needs a designation");
    assert_eq!(e("1YT"), (BAD, 3));
    assert_eq!(e("1D1Y"), (BAD, 3), "order");
    assert_eq!(e("1M1Y"), (BAD, 3));
    assert_eq!(e("1H"), (BAD, 1), "H before T");
    assert_eq!(e("T1Y"), (BAD, 2), "Y after T");
    assert_eq!(e("T1S1M"), (BAD, 4));
    assert_eq!(e("01Y"), (BAD, 0), "leading zero, X.680 §38.3.6.11");
    assert_eq!(e("1.Y"), (BAD, 2), "fraction needs a digit");
    assert_eq!(
        e("1.5Y2M"),
        (BAD, 4),
        "nothing after a fraction, X.680 §38.3.6.13"
    );
    assert_eq!(e("1.5YT1H"), (BAD, 4));
    assert_eq!(e("5W1D"), (BAD, 2), "a week stands alone");
    assert_eq!(e("1DT"), (BAD, 3));
    assert_eq!(e("1Y1Y"), (BAD, 3), "repeat");
    assert_eq!(e("TT1H"), (BAD, 1));
    assert_eq!(e("1DT2W"), (BAD, 4), "W after T");
    assert_eq!(e("1D5W"), (BAD, 3), "W after another designation");
    assert_eq!(e("T1H1D"), (BAD, 4), "D after T");
}

#[test]
fn duration_der_canonical_form() {
    // X.690 §11.9.1 a) and d); notes TM12.
    let d = |s: &str| {
        let e = time::validate_duration(s.as_bytes(), Mode::Der).unwrap_err();
        (e.class, e.offset)
    };
    assert_eq!(d("0Y29M0D"), (TF, 0), "redundant zero component");
    assert_eq!(d("1Y0M3D"), (TF, 2));
    assert_eq!(d("1DT0H5M"), (TF, 3));
    assert_eq!(d("0DT5H"), (TF, 0));
    assert_eq!(d("0.0YT5H"), (BAD, 4), "nothing follows a fraction");
    assert_eq!(d("0YT5H"), (TF, 0));
    assert_eq!(d("T0H0M0S"), (TF, 1));
    assert_eq!(
        d("1,5Y"),
        (TF, 1),
        "comma as decimal sign, X.690 §11.9.1 a)"
    );
    assert_eq!(d("T0,5S"), (TF, 2));
    // BER takes all of them
    for s in ["0Y29M0D", "1Y0M3D", "1,5Y", "T0H0M0S"] {
        assert_eq!(
            time::validate_duration(s.as_bytes(), Mode::Ber),
            Ok(()),
            "{s}"
        );
    }
    // a grammar error wins over a DER error
    assert_eq!(
        time::validate_duration(b"0Y1.Y", Mode::Der)
            .unwrap_err()
            .class,
        BAD
    );
}

#[test]
fn general_time_contents() {
    // constructed by hand from X.690 §8.26.1.2 (only the quotes are removed); notes TM05 to TM10.
    for s in [
        "2021-02-15",
        "2021-02-15T12:30:45Z",
        "2021-02-15T12:30:45+05:30",
        "2021-02-15T12:30:45+05",
        "2021-02-15T12:30:45-05",
    ] {
        for m in [Mode::Ber, Mode::Der] {
            assert_eq!(time::validate_time(s.as_bytes(), m), Ok(()), "{s}");
        }
    }
    let bad_der = |s: &str| {
        let e = time::validate_time(s.as_bytes(), Mode::Der).unwrap_err();
        (e.class, e.offset)
    };
    assert_eq!(bad_der("2021-02-15T12:30:45+05:00"), (TF, 22));
    assert_eq!(bad_der("2021-02-15T12:30:45-05:00"), (TF, 22));
    assert_eq!(bad_der("2021-02-15T12:30:45,5Z"), (TF, 19));
    assert!(time::validate_time(b"2021-02-15T12:30:45+05:00", Mode::Ber).is_ok());
    assert!(time::validate_time(b"2021-02-15T12:30:45,5Z", Mode::Ber).is_ok());
    // minutes that are not zero stay
    assert!(time::validate_time(b"2021-02-15T12:30:45+05:30", Mode::Der).is_ok());
    assert!(time::validate_time(b"T12:30+05:300", Mode::Der).is_ok());
    // not printable ASCII
    assert_eq!(
        time::validate_time(b"2021\x00", Mode::Ber)
            .unwrap_err()
            .offset,
        4
    );
    assert_eq!(
        time::validate_time(&[0x32, 0xC3, 0xA9], Mode::Ber)
            .unwrap_err()
            .offset,
        1
    );
}
