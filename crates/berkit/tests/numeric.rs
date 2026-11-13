//! BOOLEAN, INTEGER, ENUMERATED, NULL and REAL contents, through the public API.
//!
//! Vectors cite `docs/design/primitives.md` ids. Each is labelled `from X.690 <place>` or `constructed by hand`.
//! The notes' hex is for whole TLVs; these tests take the contents octets (the TLV framing is another module).
//! The notes name error classes `BadLength`, `NonMinimal`, `NonCanonical`, `RealReserved`; the crate's classes
//! are `bad_contents`, `non_minimal_integer`, `boolean_not_ff` and `real_form`, `bad_contents`.

use berkit::{ErrorClass, Mode, boolean, integer, null, real};

const BER: Mode = Mode::Ber;
const DER: Mode = Mode::Der;
const BOTH: [Mode; 2] = [BER, DER];

fn class<T: core::fmt::Debug>(r: Result<T, berkit::Error>) -> ErrorClass {
    r.expect_err("expected an error").class
}

// ---------------------------------------------------------------- BOOLEAN

#[test]
fn boolean_false_is_00_in_both_modes() {
    // V-B02, constructed by hand from X.690 §8.2.2 (FALSE is the octet zero).
    for m in BOTH {
        assert_eq!(boolean::decode(&[0x00], m), Ok(false));
    }
}

#[test]
fn boolean_true_ff_is_accepted_in_both_modes() {
    // V-B01, from X.690 §8.2.2 example (contents FF).
    for m in BOTH {
        assert_eq!(boolean::decode(&[0xFF], m), Ok(true));
    }
}

#[test]
fn boolean_ber_accepts_every_nonzero_octet_as_true() {
    // V-B03, V-B05, constructed by hand from X.690 §8.2.2 (any non-zero value is TRUE).
    for v in 1..=255u8 {
        assert_eq!(boolean::decode(&[v], BER), Ok(true), "octet {v:#04x}");
    }
}

#[test]
fn boolean_der_rejects_every_nonzero_octet_but_ff() {
    // V-B04, V-B05, constructed by hand from X.690 §11.1 (TRUE has all eight bits set).
    for v in 1..=254u8 {
        assert_eq!(
            class(boolean::decode(&[v], DER)),
            ErrorClass::BooleanNotFf,
            "octet {v:#04x}"
        );
    }
}

#[test]
fn boolean_wrong_length_is_bad_contents_in_both_modes() {
    // V-B06, V-B07, constructed by hand from X.690 §8.2.1.
    for m in BOTH {
        assert_eq!(class(boolean::decode(&[], m)), ErrorClass::BadContents);
        assert_eq!(
            class(boolean::decode(&[0xFF, 0xFF], m)),
            ErrorClass::BadContents
        );
        assert_eq!(
            class(boolean::decode(&[0x00, 0x00], m)),
            ErrorClass::BadContents
        );
        assert_eq!(
            class(boolean::decode(&[0x01, 0x01, 0x01], m)),
            ErrorClass::BadContents
        );
    }
}

#[test]
fn boolean_encode_writes_literal_ff_and_00() {
    // Constructed by hand from X.690 §8.2.2 and §11.1.
    let mut out = [0x55u8; 2];
    assert_eq!(boolean::encode(true, &mut out), Some(1));
    assert_eq!(out, [0xFF, 0x55]);
    assert_eq!(boolean::encode(false, &mut out), Some(1));
    assert_eq!(out, [0x00, 0x55]);
    assert_eq!(boolean::encode(true, &mut []), None);
}

#[test]
fn boolean_encoding_decodes_in_der() {
    for v in [false, true] {
        let mut out = [0u8; 1];
        let n = boolean::encode(v, &mut out).unwrap();
        assert_eq!(boolean::decode(&out[..n], DER), Ok(v));
    }
}

// ---------------------------------------------------------------- NULL

#[test]
fn null_empty_contents_decode_in_both_modes() {
    // V-N01, from X.690 §8.8.2 (no contents octets).
    for m in BOTH {
        assert_eq!(null::decode(&[], m), Ok(()));
    }
}

#[test]
fn null_with_contents_is_bad_contents() {
    // V-N02, constructed by hand from X.690 §8.8.2.
    for m in BOTH {
        assert_eq!(class(null::decode(&[0x00], m)), ErrorClass::BadContents);
        assert_eq!(
            class(null::decode(&[0x00, 0x00], m)),
            ErrorClass::BadContents
        );
        assert_eq!(class(null::decode(&[0xFF], m)), ErrorClass::BadContents);
    }
}

#[test]
fn null_encodes_to_zero_octets() {
    // Constructed by hand from X.690 §8.8.2.
    let mut out = [0x55u8; 1];
    assert_eq!(null::encode(&mut out), 0);
    assert_eq!(out, [0x55]);
}

// ---------------------------------------------------------------- INTEGER

/// Contents and value, from the vectors of section 2.2 (V-I01 to V-I14, V-E01 to V-E03), constructed by hand
/// unless noted. Values recomputed here from two's complement, not copied: see `integer_vectors_recomputed`.
const INT_VECTORS: &[(&[u8], i128)] = &[
    (&[0x00], 0),                 // V-I01, V-E01
    (&[0x7F], 127),               // V-I02
    (&[0x00, 0x80], 128),         // V-I03
    (&[0x80], -128),              // V-I04
    (&[0xFF, 0x7F], -129),        // V-I05, minimal (not the FF 80 form)
    (&[0x00, 0xFF], 255),         // V-I06
    (&[0x01, 0x00], 256),         // V-I07
    (&[0xFF], -1),                // V-I08, V-E03
    (&[0x7F, 0xFF], 32767),       // V-I09
    (&[0x80, 0x00], -32768),      // V-I10
    (&[0x00, 0x80, 0x00], 32768), // V-I11
    (&[0x33], 51),                // V-I12, from X.690 Annex A.3 (content 33)
    (&[0x02], 2),                 // V-E02
    (
        &[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        1 << 64,
    ), // V-I13
    (
        &[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        (1 << 64) - 1,
    ), // V-I14
];

#[test]
fn integer_vectors_decode_to_their_values_in_both_modes() {
    for &(bytes, value) in INT_VECTORS {
        for m in BOTH {
            let i = integer::decode(bytes, m).unwrap();
            assert_eq!(i.to_i128(), Some(value), "{bytes:02X?}");
            assert_eq!(i.as_bytes(), bytes);
        }
    }
}

#[test]
fn integer_vectors_recomputed() {
    // Independent arithmetic for each vector: sum of octet x 256^k with the top octet signed.
    for &(bytes, value) in INT_VECTORS {
        let mut v: i128 = i128::from(i8::from_be_bytes([bytes[0]]));
        for &b in &bytes[1..] {
            v = v * 256 + i128::from(b);
        }
        assert_eq!(v, value, "{bytes:02X?}");
    }
}

#[test]
fn integer_vectors_encode_to_the_literal_wire_bytes() {
    for &(bytes, value) in INT_VECTORS {
        let mut out = [0u8; 20];
        let n = integer::encode_i128(value, &mut out).unwrap();
        assert_eq!(&out[..n], bytes, "{value}");
        if let Ok(v) = i64::try_from(value) {
            let n = integer::encode_i64(v, &mut out).unwrap();
            assert_eq!(&out[..n], bytes, "{value}");
        }
        if let Ok(v) = u64::try_from(value) {
            let n = integer::encode_u64(v, &mut out).unwrap();
            assert_eq!(&out[..n], bytes, "{value}");
        }
        let mut mag = [0u8; 20];
        let k = integer::decode(bytes, BER)
            .unwrap()
            .write_magnitude(&mut mag)
            .unwrap();
        let n = integer::encode_magnitude(value < 0, &mag[..k], &mut out).unwrap();
        assert_eq!(&out[..n], bytes, "{value}");
    }
}

#[test]
fn integer_empty_contents_is_bad_contents() {
    // V-I15, V-E04, constructed by hand from X.690 §8.3.1 (one or more octets).
    for m in BOTH {
        assert_eq!(class(integer::decode(&[], m)), ErrorClass::BadContents);
    }
}

#[test]
fn integer_non_minimal_forms_are_rejected_in_ber_and_der() {
    // V-I16 to V-I20, V-E05, constructed by hand from X.690 §8.3.2 a) and b).
    let cases: &[&[u8]] = &[
        &[0x00, 0x7F],
        &[0xFF, 0x80],
        &[0x00, 0x00],
        &[0xFF, 0xFF],
        &[0x00, 0x00, 0x80],
        &[0x00, 0x05],
        &[0xFF, 0xFF, 0x7F],
        &[0x00, 0x00, 0x00],
        &[0xFF, 0x80, 0x00],
        &[0x00, 0x01, 0x00],
    ];
    for bytes in cases {
        for m in BOTH {
            let e = integer::decode(bytes, m).unwrap_err();
            assert_eq!(e.class, ErrorClass::NonMinimalInteger, "{bytes:02X?}");
            assert_eq!(e.offset, 0);
        }
    }
}

#[test]
fn integer_boundary_pairs_that_look_non_minimal_are_minimal() {
    // From X.690 §8.3.2 a) and b): the second octet's bit 8 decides. Constructed by hand.
    let minimal: &[&[u8]] = &[
        &[0x00, 0x80],
        &[0x00, 0xFF],
        &[0xFF, 0x7F],
        &[0xFF, 0x00],
        &[0x01, 0x00],
    ];
    for bytes in minimal {
        assert!(integer::decode(bytes, DER).is_ok(), "{bytes:02X?}");
    }
}

#[test]
fn integer_conversions_at_the_i64_edges() {
    // Constructed by hand from X.690 §8.3.3.
    let max = [0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let min = [0x80, 0, 0, 0, 0, 0, 0, 0];
    let over = [0x00, 0x80, 0, 0, 0, 0, 0, 0, 0]; // 2^63
    let under = [0xFF, 0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]; // -2^63 - 1
    let i = |b: &[u8]| integer::decode(b, BER).map(|x| (x.to_i64(), x.to_u64()));
    assert_eq!(
        i(&max),
        Ok((Some(i64::MAX), Some(u64::try_from(i64::MAX).unwrap())))
    );
    assert_eq!(i(&min), Ok((Some(i64::MIN), None)));
    assert_eq!(i(&over), Ok((None, Some(1 << 63))));
    assert_eq!(i(&under), Ok((None, None)));
}

#[test]
fn integer_conversions_at_the_u64_edges() {
    // V-I13, V-I14 and neighbours, constructed by hand.
    let max = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let over = [0x01, 0, 0, 0, 0, 0, 0, 0, 0];
    let neg = [0xFF];
    let i = |b: &[u8]| {
        integer::decode(b, DER).map(|x| (x.to_i64(), x.to_u64(), x.to_u128(), x.to_i128()))
    };
    assert_eq!(
        i(&max),
        Ok((
            None,
            Some(u64::MAX),
            Some(u128::from(u64::MAX)),
            Some(i128::from(u64::MAX))
        ))
    );
    assert_eq!(i(&over), Ok((None, None, Some(1 << 64), Some(1 << 64))));
    assert_eq!(i(&neg), Ok((Some(-1), None, None, Some(-1))));
}

#[test]
fn integer_300_octets_is_carried_without_truncation() {
    // Constructed by hand: 0x01 then 299 zero octets, i.e. 2^(8 x 299). Larger than any fixed-width type.
    let mut big = [0u8; 300];
    big[0] = 0x01;
    let i = integer::decode(&big, DER).unwrap();
    assert_eq!(i.as_bytes().len(), 300);
    assert_eq!(
        (i.to_i64(), i.to_u64(), i.to_i128(), i.to_u128()),
        (None, None, None, None)
    );
    assert!(!i.is_negative());
    assert_eq!(i.unsigned_bytes(), Some(&big[..]));
    // Negative 300 octets: 0x80 then 299 zeros is -2^(8 x 300 - 1); its magnitude is 0x80 then zeros.
    big[0] = 0x80;
    let n = integer::decode(&big, BER).unwrap();
    assert!(n.is_negative());
    assert_eq!(n.unsigned_bytes(), None);
    let mut mag = [0u8; 300];
    assert_eq!(n.write_magnitude(&mut mag), Some(300));
    assert_eq!(mag[0], 0x80);
    assert!(mag[1..].iter().all(|&b| b == 0));
    // And it re-encodes to the same 300 octets.
    let mut out = [0u8; 301];
    assert_eq!(integer::encode_magnitude(true, &mag, &mut out), Some(300));
    assert_eq!(&out[..300], &big);
}

#[test]
fn integer_9_octets_over_u64_and_i64_do_not_fit() {
    // V-I13 as a 9-octet value; also a 9-octet negative.
    let nine_neg = [0xFE, 0, 0, 0, 0, 0, 0, 0, 0];
    let i = integer::decode(&nine_neg, BER).unwrap();
    assert_eq!(i.to_i64(), None);
    assert_eq!(i.to_i128(), Some(-(2i128 << 64)));
}

#[test]
fn integer_17_octets_do_not_fit_i128_or_u128() {
    // Constructed by hand: 2^128 is 0x01 followed by 16 zero octets.
    let mut b = [0u8; 17];
    b[0] = 1;
    let i = integer::decode(&b, BER).unwrap();
    assert_eq!((i.to_i128(), i.to_u128()), (None, None));
    // 2^127 fits u128 but not i128 (0x00 0x80 ...).
    let mut c = [0u8; 17];
    c[1] = 0x80;
    let j = integer::decode(&c, BER).unwrap();
    assert_eq!((j.to_i128(), j.to_u128()), (None, Some(1u128 << 127)));
}

#[test]
fn integer_zero_helpers() {
    let z = integer::decode(&[0], BER).unwrap();
    assert!(z.is_zero() && !z.is_negative());
    assert_eq!(z.unsigned_bytes(), Some(&[0u8][..]));
    let mut m = [0xAAu8; 4];
    assert_eq!(z.write_magnitude(&mut m), Some(1));
    assert_eq!(m[0], 0);
    assert!(!integer::decode(&[1], BER).unwrap().is_zero());
}

#[test]
fn integer_encode_i64_literal_wire_bytes() {
    // Constructed by hand from X.690 §8.3.2 and §8.3.3.
    let cases: &[(i64, &[u8])] = &[
        (0, &[0x00]),
        (127, &[0x7F]),
        (128, &[0x00, 0x80]),
        (-128, &[0x80]),
        (-129, &[0xFF, 0x7F]),
        (255, &[0x00, 0xFF]),
        (256, &[0x01, 0x00]),
        (32767, &[0x7F, 0xFF]),
        (32768, &[0x00, 0x80, 0x00]),
        (-32768, &[0x80, 0x00]),
        (-32769, &[0xFF, 0x7F, 0xFF]),
        (-1, &[0xFF]),
        (-256, &[0xFF, 0x00]),
        (i64::MAX, &[0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
        (i64::MIN, &[0x80, 0, 0, 0, 0, 0, 0, 0]),
    ];
    for &(v, want) in cases {
        let mut out = [0u8; 16];
        let n = integer::encode_i64(v, &mut out).unwrap();
        assert_eq!(&out[..n], want, "{v}");
        assert_eq!(integer::decode(&out[..n], DER).unwrap().to_i64(), Some(v));
    }
}

#[test]
fn integer_encode_u64_literal_wire_bytes() {
    let cases: &[(u64, &[u8])] = &[
        (0, &[0x00]),
        (127, &[0x7F]),
        (128, &[0x00, 0x80]),
        (255, &[0x00, 0xFF]),
        (
            u64::try_from(i64::MAX).unwrap(),
            &[0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        ),
        (1 << 63, &[0x00, 0x80, 0, 0, 0, 0, 0, 0, 0]),
        (
            u64::MAX,
            &[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        ),
    ];
    for &(v, want) in cases {
        let mut out = [0u8; 16];
        let n = integer::encode_u64(v, &mut out).unwrap();
        assert_eq!(&out[..n], want, "{v}");
        assert_eq!(integer::decode(&out[..n], DER).unwrap().to_u64(), Some(v));
    }
}

#[test]
fn integer_encode_magnitude_signs_and_minimality() {
    // Constructed by hand from X.690 §8.3.2/§8.3.3: (negative, magnitude, expected contents).
    let cases: &[(bool, &[u8], &[u8])] = &[
        (false, &[], &[0x00]),
        (true, &[], &[0x00]),
        (true, &[0, 0], &[0x00]),
        (false, &[0x7F], &[0x7F]),
        (false, &[0x80], &[0x00, 0x80]),
        (false, &[0x00, 0x00, 0x7F], &[0x7F]),
        (true, &[0x80], &[0x80]),
        (true, &[0x81], &[0xFF, 0x7F]),
        (true, &[0x01], &[0xFF]),
        (true, &[0x01, 0x00], &[0xFF, 0x00]),
        (true, &[0x01, 0x01], &[0xFE, 0xFF]),
        (true, &[0x80, 0x00], &[0x80, 0x00]),
        (true, &[0x80, 0x01], &[0xFF, 0x7F, 0xFF]),
        (true, &[0x00, 0x80], &[0x80]),
        (false, &[0xFF, 0xFF], &[0x00, 0xFF, 0xFF]),
    ];
    for &(neg, mag, want) in cases {
        let mut out = [0u8; 8];
        let n = integer::encode_magnitude(neg, mag, &mut out).unwrap();
        assert_eq!(&out[..n], want, "neg={neg} mag={mag:02X?}");
        assert!(integer::decode(&out[..n], DER).is_ok());
    }
}

#[test]
fn integer_encoders_report_a_short_buffer() {
    let mut one = [0u8; 1];
    assert_eq!(integer::encode_i64(128, &mut one), None);
    assert_eq!(integer::encode_u64(u64::MAX, &mut [0u8; 8]), None);
    assert_eq!(integer::encode_magnitude(false, &[0x80], &mut one), None);
    assert_eq!(integer::encode_i64(127, &mut one), Some(1));
    assert_eq!(integer::encode_i64(0, &mut []), None);
    let i = integer::decode(&[0x00, 0x80], BER).unwrap();
    assert_eq!(i.write_magnitude(&mut one), None);
}

#[test]
fn integer_never_emits_a_non_minimal_encoding() {
    // Every i16 and a sweep of larger values encode to something the decoder accepts in both modes.
    for v in i16::MIN..=i16::MAX {
        let mut out = [0u8; 16];
        let n = integer::encode_i64(i64::from(v), &mut out).unwrap();
        for m in BOTH {
            assert_eq!(
                integer::decode(&out[..n], m).unwrap().to_i64(),
                Some(i64::from(v))
            );
        }
    }
}

// ---------------------------------------------------------------- REAL

fn kind(c: &[u8], m: Mode) -> real::Kind<'_> {
    real::decode(c, m).unwrap().kind()
}

#[test]
fn real_plus_zero_is_empty_contents() {
    // V-F01, constructed by hand from X.690 §8.5.2.
    for m in BOTH {
        let r = real::decode(&[], m).unwrap();
        assert_eq!(r.kind(), real::Kind::Zero);
        assert_eq!(r.contents(), &[] as &[u8]);
    }
}

#[test]
fn real_special_values_classify() {
    // V-F02 to V-F05, constructed by hand from X.690 §8.5.9 and §8.5.3.
    for m in BOTH {
        assert_eq!(kind(&[0x40], m), real::Kind::PlusInfinity);
        assert_eq!(kind(&[0x41], m), real::Kind::MinusInfinity);
        assert_eq!(kind(&[0x42], m), real::Kind::NotANumber);
        assert_eq!(kind(&[0x43], m), real::Kind::MinusZero);
    }
}

#[test]
fn real_special_encoders_write_literal_octets() {
    let mut out = [0u8; 1];
    for (v, want) in [
        (real::Special::PlusInfinity, 0x40),
        (real::Special::MinusInfinity, 0x41),
        (real::Special::NotANumber, 0x42),
        (real::Special::MinusZero, 0x43),
    ] {
        assert_eq!(real::encode_special(v, &mut out), Some(1));
        assert_eq!(out, [want]);
    }
    assert_eq!(
        real::encode_special(real::Special::NotANumber, &mut []),
        None
    );
}

#[test]
fn real_reserved_special_values_and_lengths_are_bad_contents() {
    // V-F06, V-F07, constructed by hand from X.690 §8.5.9 (only 40 to 43; exactly one octet).
    for m in BOTH {
        for b in 0x44..=0x7F {
            assert_eq!(
                class(real::decode(&[b], m)),
                ErrorClass::BadContents,
                "{b:#04x}"
            );
        }
        assert_eq!(
            class(real::decode(&[0x40, 0x00], m)),
            ErrorClass::BadContents
        );
        assert_eq!(
            class(real::decode(&[0x43, 0x43], m)),
            ErrorClass::BadContents
        );
    }
}

#[test]
fn real_binary_fields_are_split_without_loss() {
    // V-F08, V-F09, constructed by hand from X.690 §8.5.7 to §8.5.7.5.
    let c = [0x80, 0x00, 0x01];
    assert_eq!(
        kind(&c, BER),
        real::Kind::Binary {
            negative: false,
            base: real::Base::Two,
            scale: 0,
            exponent: &[0x00],
            mantissa: &[0x01]
        }
    );
    let c = [0xC0, 0xFF, 0x05];
    assert_eq!(
        kind(&c, DER),
        real::Kind::Binary {
            negative: true,
            base: real::Base::Two,
            scale: 0,
            exponent: &[0xFF],
            mantissa: &[0x05]
        }
    );
    // Base 16, F = 3, two exponent octets, three mantissa octets: first octet 1010 1101 = AD.
    let c = [0xAD, 0x01, 0x02, 0x0A, 0x0B, 0x0C];
    assert_eq!(
        kind(&c, BER),
        real::Kind::Binary {
            negative: false,
            base: real::Base::Sixteen,
            scale: 3,
            exponent: &[0x01, 0x02],
            mantissa: &[0x0A, 0x0B, 0x0C]
        }
    );
}

#[test]
fn real_binary_non_normalised_forms_are_valid_ber_and_real_form_in_der() {
    // Constructed by hand from X.690 §8.5.7.2, §8.5.7.3, §8.5.7.4, §8.5.7.5 NOTE 1 (BER) and §11.3.1 (DER).
    let cases: &[(&str, &[u8])] = &[
        ("V-F10 N even", &[0x80, 0x00, 0x02]),
        ("V-F11 base 8", &[0x90, 0x00, 0x01]),
        ("base 16", &[0xA0, 0x00, 0x01]),
        ("V-F12 F=1", &[0x84, 0x00, 0x01]),
        ("V-F13 exponent not minimal", &[0x81, 0x00, 0x01, 0x01]),
        ("V-F14 N leading 00", &[0x80, 0x00, 0x00, 0x01]),
        ("V-F17 form 11 with X=2", &[0x83, 0x02, 0x00, 0x80, 0x01]),
        (
            "exponent 3 octets minimal needed 2",
            &[0x82, 0x00, 0x00, 0x01, 0x01],
        ),
        ("no mantissa", &[0x80, 0x00]),
        ("form 11, X=1", &[0x83, 0x01, 0x05, 0x01]),
        ("form 11, X=3", &[0x83, 0x03, 0x01, 0x00, 0x00, 0x01]),
    ];
    for &(name, c) in cases {
        let r = real::decode(c, BER).unwrap_or_else(|e| panic!("{name}: BER {e}"));
        assert_eq!(r.contents(), c, "{name}");
        assert_eq!(class(real::decode(c, DER)), ErrorClass::RealForm, "{name}");
    }
}

#[test]
fn real_der_accepts_canonical_binary() {
    // Constructed by hand from X.690 §11.3.1: base 2, F = 0, odd N, minimal exponent and N.
    let cases: &[&[u8]] = &[
        &[0x80, 0x00, 0x01],                         // 1
        &[0xC0, 0xFF, 0x05],                         // -2.5
        &[0x80, 0x7F, 0x01],                         // exponent +127
        &[0x80, 0x80, 0x01],                         // exponent -128
        &[0x81, 0x00, 0x80, 0x01],                   // exponent 128 in two octets
        &[0x81, 0xFF, 0x7F, 0x01],                   // exponent -129
        &[0x82, 0x01, 0x00, 0x00, 0x03],             // exponent 65536 in three octets
        &[0x83, 0x04, 0x01, 0x00, 0x00, 0x00, 0x07], // exponent 2^24 in four octets, form 11
        &[0x80, 0x00, 0x01, 0x00, 0x01],             // N = 0x0101, odd, no leading 00
        &[0x80, 0x01, 0xFF],                         // exponent 1, N = 255
    ];
    for &c in cases {
        let r = real::decode(c, DER).unwrap_or_else(|e| panic!("{c:02X?}: {e}"));
        assert_eq!(r.contents(), c);
        assert_eq!(real::decode(c, BER).unwrap(), r);
    }
}

#[test]
fn real_reserved_binary_fields_are_bad_contents_in_both_modes() {
    // V-F15, V-F16, constructed by hand from X.690 §8.5.7.2 and §8.5.7.4 d).
    let cases: &[(&str, &[u8])] = &[
        ("V-F15 base bits 11", &[0xB0, 0x00, 0x01]),
        ("form 11, X=0", &[0x83, 0x00, 0x01]),
        ("form 11, X missing", &[0x83]),
        (
            "V-F16 form 11, X=2, nine zero bits",
            &[0x83, 0x02, 0x00, 0x01, 0x01],
        ),
        (
            "form 11, X=2, nine one bits",
            &[0x83, 0x02, 0xFF, 0x80, 0x01],
        ),
        (
            "form 11, X=3, nine zero bits",
            &[0x83, 0x03, 0x00, 0x7F, 0x00, 0x01],
        ),
        ("exponent octet missing, format 00", &[0x80]),
        ("exponent short, format 01", &[0x81, 0x00]),
        ("exponent short, format 10", &[0x82, 0x00, 0x00]),
        ("form 11, exponent short", &[0x83, 0x03, 0x01, 0x02]),
    ];
    for &(name, c) in cases {
        for m in BOTH {
            assert_eq!(
                class(real::decode(c, m)),
                ErrorClass::BadContents,
                "{name} {m:?}"
            );
        }
    }
}

#[test]
fn real_form_11_with_one_exponent_octet_is_accepted_in_ber_open_row() {
    // The notes list this as not stated: with X = 1 the exponent has eight bits, "first nine bits" cannot be
    // examined. POLICY: accept in BER, carried opaquely. Constructed by hand.
    for e in [0x00, 0x01, 0xFF, 0x80, 0x7F] {
        let c = [0x83, 0x01, e, 0x01];
        let r = real::decode(&c, BER).unwrap();
        assert_eq!(r.contents(), &c);
    }
}

#[test]
fn real_decimal_forms_classify() {
    // V-F18, V-F19, constructed by hand from X.690 §8.5.8.
    assert_eq!(
        kind(&[0x01, b'1', b'5'], BER),
        real::Kind::Decimal {
            form: real::DecimalForm::Nr1,
            field: b"15"
        }
    );
    assert_eq!(
        kind(&[0x02, b'1', b'.', b'5'], BER),
        real::Kind::Decimal {
            form: real::DecimalForm::Nr2,
            field: b"1.5"
        }
    );
    for m in BOTH {
        assert_eq!(
            kind(&[0x03, 0x31, 0x35, 0x2E, 0x45, 0x2D, 0x31], m),
            real::Kind::Decimal {
                form: real::DecimalForm::Nr3,
                field: b"15.E-1"
            }
        );
    }
}

#[test]
fn real_reserved_decimal_forms_are_bad_contents() {
    // V-F20, constructed by hand from X.690 §8.5.8 (bits 6 to 1 other than 1, 2, 3 are reserved).
    for m in BOTH {
        for b in (0x00..=0x3F).filter(|b| !(1..=3).contains(b)) {
            assert_eq!(
                class(real::decode(&[b, b'1'], m)),
                ErrorClass::BadContents,
                "{b:#04x}"
            );
        }
    }
}

#[test]
fn real_der_requires_nr3() {
    // V-F18, constructed by hand from X.690 §11.3.2.1.
    assert!(real::decode(&[0x02, 0x31, 0x2E, 0x35], BER).is_ok());
    assert_eq!(
        class(real::decode(&[0x02, 0x31, 0x2E, 0x35], DER)),
        ErrorClass::RealForm
    );
    // NR1 and NR2 first octets over a field that is valid NR3 text: the first octet alone decides.
    for first in [0x01, 0x02] {
        let c = [first, b'1', b'.', b'E', b'+', b'0'];
        assert!(real::decode(&c, BER).is_ok());
        assert_eq!(class(real::decode(&c, DER)), ErrorClass::RealForm);
    }
    assert_eq!(
        class(real::decode(&[0x01, 0x31], DER)),
        ErrorClass::RealForm
    );
}

#[test]
fn real_der_nr3_syntax() {
    // Constructed by hand from X.690 §11.3.2.2 to §11.3.2.6.
    let good: &[&str] = &[
        "15.E-1", "1.E+0", "-1.E+0", "123.E5", "5.E-10", "1.E10", "-25.E-3", "101.E+0",
    ];
    for g in good {
        let mut c = vec![0x03];
        c.extend_from_slice(g.as_bytes());
        assert!(real::decode(&c, DER).is_ok(), "{g}");
    }
    let bad: &[(&str, &str)] = &[
        ("1 .E+0", "SPACE, 11.3.2.2"),
        (" 1.E+0", "leading SPACE, 11.3.2.2"),
        ("+1.E+0", "plus sign, 11.3.2.3"),
        ("05.E+0", "first mantissa digit 0, 11.3.2.4"),
        ("0.E+0", "mantissa 0, 11.3.2.4"),
        ("10.E+0", "last mantissa digit 0, 11.3.2.4"),
        ("-10.E+0", "negative, last digit 0"),
        ("-0.E+0", "negative, first digit 0"),
        ("1E+0", "no FULL STOP, 11.3.2.5"),
        ("1.e+0", "lower-case exponent mark, 11.3.2.5"),
        ("1.5E+0", "decimal mark inside mantissa, 11.3.2.5"),
        ("1.E0", "zero exponent without plus, 11.3.2.6"),
        ("1.E-0", "zero exponent with minus, 11.3.2.6"),
        ("1.E00", "leading zero exponent, 11.3.2.6"),
        ("1.E+5", "plus on non-zero exponent, 11.3.2.6"),
        ("1.E05", "leading zero exponent digit, 11.3.2.6"),
        ("1.E", "no exponent"),
        ("1.E-", "sign only"),
        ("1.E1 ", "trailing SPACE"),
        ("1.E1x", "stray letter"),
        ("", "empty field"),
        ("-", "sign only mantissa"),
        (".E+0", "no digits"),
    ];
    for (b, why) in bad {
        let mut c = vec![0x03];
        c.extend_from_slice(b.as_bytes());
        assert_eq!(
            class(real::decode(&c, DER)),
            ErrorClass::RealForm,
            "{b:?}: {why}"
        );
        assert!(real::decode(&c, BER).is_ok(), "BER carries {b:?} opaquely");
    }
}

#[test]
fn real_decode_then_write_is_byte_for_byte() {
    // Constructed by hand: every encoding above, including the ones DER refuses, writes back unchanged.
    let all: &[&[u8]] = &[
        &[],
        &[0x40],
        &[0x43],
        &[0x80, 0x00, 0x02],
        &[0x84, 0x00, 0x00, 0x01],
        &[0x83, 0x02, 0x00, 0x80, 0x01],
        &[0xAD, 0x01, 0x02, 0x0A, 0x0B, 0x0C],
        &[0x02, 0x31, 0x2E, 0x35],
        &[0x03, 0x31, 0x35, 0x2E, 0x45, 0x2D, 0x31],
        &[0x80, 0x00],
    ];
    for &c in all {
        let r = real::decode(c, BER).unwrap();
        let mut out = [0u8; 16];
        let n = r.write(&mut out).unwrap();
        assert_eq!(&out[..n], c);
        assert_eq!(
            r.write(&mut out[..c.len().saturating_sub(1)]).is_none(),
            !c.is_empty()
        );
    }
}
