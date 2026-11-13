//! Integration tests for the `oid` module. Vector provenance is labelled `from X.690 <place>` for a value the
//! standard prints and `constructed by hand` for the rest; every byte string here was recomputed
//! independently (a base-128 encoder written apart from the crate) before it was written down.

use berkit::oid::{
    Arc, EncodeError, ObjectIdentifier, OidIri, RelativeOid, RelativeOidIri,
    encode_object_identifier_be, encode_object_identifier_u64, encode_object_identifier_u128,
    encode_relative_oid_be, encode_relative_oid_u64, encode_relative_oid_u128,
};
use berkit::{Error, ErrorClass, Mode};

const MODES: [Mode; 2] = [Mode::Ber, Mode::Der];

/// Arcs as `u128`, `None` where an arc does not fit.
fn arcs_of<'a>(it: impl Iterator<Item = Arc<'a>>) -> Vec<Option<u128>> {
    it.map(|a| a.to_u128()).collect()
}

fn oid_arcs(content: &[u8]) -> Vec<Option<u128>> {
    let oid = ObjectIdentifier::decode(content, Mode::Ber).unwrap();
    arcs_of(oid.arcs())
}

fn some(v: &[u128]) -> Vec<Option<u128>> {
    v.iter().map(|&x| Some(x)).collect()
}

fn enc(arcs: &[u128]) -> Vec<u8> {
    let mut buf = [0u8; 64];
    let n = encode_object_identifier_u128(arcs, &mut buf).unwrap();
    buf[..n].to_vec()
}

fn enc_rel(arcs: &[u128]) -> Vec<u8> {
    let mut buf = [0u8; 64];
    let n = encode_relative_oid_u128(arcs, &mut buf).unwrap();
    buf[..n].to_vec()
}

fn err_class<T: core::fmt::Debug>(r: Result<T, Error>) -> ErrorClass {
    r.unwrap_err().class
}

// ---- OBJECT IDENTIFIER: values ------------------------------------------------------------------------

#[test]
fn the_x690_example_2_999_3_is_88_37_03() {
    // from X.690 §8.19.5 (02/2021): {2 999 3}, first subidentifier 1079, contents 88 37 03.
    let content = [0x88, 0x37, 0x03];
    assert_eq!(oid_arcs(&content), some(&[2, 999, 3]));
    assert_eq!(enc(&[2, 999, 3]), content);
}

#[test]
fn the_2008_example_2_100_3_is_81_34_03() {
    // from X.690 (11/2008) §8.19.5 example, not in the 02/2021 text. Check: 2*40 + 100 = 180 = 1*128 + 52.
    let content = [0x81, 0x34, 0x03];
    assert_eq!(oid_arcs(&content), some(&[2, 100, 3]));
    assert_eq!(enc(&[2, 100, 3]), content);
}

#[test]
fn first_two_arcs_split_at_40_and_80() {
    // constructed by hand from §8.19.4: value = X*40 + Y, so 0..=39 is 0.Y, 40..=79 is 1.Y, 80 up is 2.Y.
    let table: [(&[u8], [u128; 2]); 8] = [
        (&[0x00], [0, 0]),
        (&[0x27], [0, 39]),
        (&[0x28], [1, 0]),
        (&[0x4F], [1, 39]),
        (&[0x50], [2, 0]),
        (&[0x7F], [2, 47]),
        (&[0x81, 0x00], [2, 48]),
        (&[0x51], [2, 1]),
    ];
    for (content, arcs) in table {
        assert_eq!(oid_arcs(content), some(&arcs), "{content:02X?}");
        assert_eq!(enc(&arcs), content, "{arcs:?}");
    }
}

#[test]
fn common_values() {
    // constructed by hand: 840 = 6*128 + 72 = 86 48; 113549 = 6*16384 + 119*128 + 13 = 86 F7 0D.
    let rsa = [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D];
    assert_eq!(oid_arcs(&rsa), some(&[1, 2, 840, 113_549]));
    assert_eq!(enc(&[1, 2, 840, 113_549]), rsa);
    let rsa_enc = [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01];
    assert_eq!(oid_arcs(&rsa_enc), some(&[1, 2, 840, 113_549, 1, 1]));
    // constructed by hand: 2*40 + 5 = 85 = 0x55.
    let cn = [0x55, 0x04, 0x03];
    assert_eq!(oid_arcs(&cn), some(&[2, 5, 4, 3]));
    assert_eq!(enc(&[2, 5, 4, 3]), cn);
}

#[test]
fn annex_b_values_contents() {
    // Annex B lists the arcs; the bytes are constructed by hand: 2*40 + 1 = 81 = 0x51.
    let ber = [0x51, 0x01];
    let cer = [0x51, 0x02, 0x00];
    let der = [0x51, 0x02, 0x01];
    assert_eq!(oid_arcs(&ber), some(&[2, 1, 1]));
    assert_eq!(oid_arcs(&cer), some(&[2, 1, 2, 0]));
    assert_eq!(oid_arcs(&der), some(&[2, 1, 2, 1]));
    assert_eq!(enc(&[2, 1, 1]), ber);
    assert_eq!(enc(&[2, 1, 2, 0]), cer);
    assert_eq!(enc(&[2, 1, 2, 1]), der);
}

#[test]
fn single_subidentifier_is_two_arcs() {
    // constructed by hand: one subidentifier 0x2A carries both the first and the second arc.
    assert_eq!(oid_arcs(&[0x2A]), some(&[1, 2]));
}

// ---- OBJECT IDENTIFIER: arc sizes ---------------------------------------------------------------------

#[test]
fn arcs_around_two_pow_32_and_64_never_truncate() {
    // constructed by hand from §8.19.2: 2^32 is five groups 16,0,0,0,0.
    let a32 = [0x2A, 0x90, 0x80, 0x80, 0x80, 0x00];
    assert_eq!(oid_arcs(&a32), some(&[1, 2, 1 << 32]));
    assert_eq!(enc(&[1, 2, 1 << 32]), a32);
    // 2^64 - 1 is one group of 1 then nine of 127.
    let max64 = [
        0x2A, 0x81, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
    ];
    let oid = ObjectIdentifier::decode(&max64, Mode::Ber).unwrap();
    assert_eq!(oid.arcs().nth(2).unwrap().to_u64(), Some(u64::MAX));
    assert_eq!(enc(&[1, 2, u128::from(u64::MAX)]), max64);
    // 2^64 does not fit a u64, and must not wrap to 0.
    let two64 = [
        0x2A, 0x82, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00,
    ];
    let oid = ObjectIdentifier::decode(&two64, Mode::Ber).unwrap();
    let arc = oid.arcs().nth(2).unwrap();
    assert_eq!(arc.to_u64(), None);
    assert_eq!(arc.to_u128(), Some(1u128 << 64));
    assert_eq!(enc(&[1, 2, 1u128 << 64]), two64);
}

#[test]
fn second_arc_under_2_above_2_pow_32() {
    // constructed by hand from §8.19.4: 2^32 + 80 = 4294967376 = 90 80 80 80 50.
    let content = [0x90, 0x80, 0x80, 0x80, 0x50];
    assert_eq!(oid_arcs(&content), some(&[2, 1 << 32]));
    assert_eq!(enc(&[2, 1 << 32]), content);
}

#[test]
fn uuid_arc_under_2_25_is_128_bits() {
    // constructed by hand: 2.25.<UUID f81d4fae-7dec-11d0-a765-00a0c91e6bf6 read as a 128-bit integer>;
    // 2*40 + 25 = 105 = 0x69.
    let uuid: u128 = 0xf81d_4fae_7dec_11d0_a765_00a0_c91e_6bf6;
    let content = [
        0x69, 0x83, 0xF0, 0x9D, 0xA7, 0xEB, 0xCF, 0xDE, 0xE0, 0xC7, 0xA1, 0xA7, 0xB2, 0xC0, 0x94,
        0x8C, 0xC8, 0xF9, 0xD7, 0x76,
    ];
    assert_eq!(oid_arcs(&content), some(&[2, 25, uuid]));
    assert_eq!(enc(&[2, 25, uuid]), content);
}

#[test]
fn largest_second_arc_under_2_needs_the_carry_past_128_bits() {
    // constructed by hand: 2.(2^128 - 1): the subidentifier is 2^128 + 79, 129 bits: 84, sixteen 80, 4F.
    let content = [
        0x84, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
        0x80, 0x80, 0x80, 0x4F,
    ];
    assert_eq!(oid_arcs(&content), some(&[2, u128::MAX]));
    assert_eq!(enc(&[2, u128::MAX]), content);
    // One more is 2^128: to_u128 must say None, and be_bytes must give the 17 octets.
    let mut over = content.to_vec();
    *over.last_mut().unwrap() = 0x50;
    let oid = ObjectIdentifier::decode(&over, Mode::Ber).unwrap();
    let second = oid.arcs().nth(1).unwrap();
    assert_eq!(second.to_u128(), None);
    let mut be = [0u8; 32];
    let n = second.be_bytes(&mut be).unwrap();
    let mut want = [0u8; 17];
    want[0] = 1;
    assert_eq!(&be[..n], &want);
}

#[test]
fn be_bytes_gives_exact_values_including_the_bias() {
    let mut buf = [0u8; 32];
    // 840 = 0x0348; 2^64 = 01 followed by eight zero octets; zero is one octet.
    let cases: [(&[u8], usize, &[u8]); 5] = [
        (&[0x2A, 0x86, 0x48], 2, &[0x03, 0x48]),
        (
            &[
                0x2A, 0x82, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00,
            ],
            2,
            &[1, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (&[0x2A, 0x00], 2, &[0]),
        (&[0x81, 0x34, 0x03], 1, &[100]), // second arc 100 under 2: 180 - 80
        (&[0x90, 0x80, 0x80, 0x80, 0x50], 1, &[1, 0, 0, 0, 0]), // 2^32 + 80 - 80
    ];
    for (content, idx, want) in cases {
        let oid = ObjectIdentifier::decode(content, Mode::Ber).unwrap();
        let arc = oid.arcs().nth(idx).unwrap();
        let n = arc.be_bytes(&mut buf).unwrap();
        assert_eq!(&buf[..n], want, "{content:02X?}");
    }
    // A buffer that is too small is refused rather than filled partially.
    let oid = ObjectIdentifier::decode(&[0x2A, 0x86, 0x48], Mode::Ber).unwrap();
    let arc = oid.arcs().nth(2).unwrap();
    assert_eq!(arc.be_bytes(&mut [0u8; 1]), None);
}

#[test]
fn arc_octets_are_the_subidentifier() {
    let oid = ObjectIdentifier::decode(&[0x88, 0x37, 0x03, 0x86, 0x48], Mode::Ber).unwrap();
    let octets: Vec<&[u8]> = oid.arcs().map(|a| a.octets()).collect();
    // The first two arcs share the first subidentifier (§8.19.4).
    assert_eq!(
        octets,
        [&[0x88, 0x37][..], &[0x88, 0x37], &[0x03], &[0x86, 0x48]]
    );
}

// ---- OBJECT IDENTIFIER: refusals ----------------------------------------------------------------------

#[test]
fn leading_80_is_refused_in_ber_and_der() {
    // constructed by hand from §8.19.2 ¶6.
    for mode in MODES {
        let e = ObjectIdentifier::decode(&[0x2A, 0x80, 0x01], mode).unwrap_err();
        assert_eq!(e, Error::new(ErrorClass::NonMinimalSubidentifier, 1));
        let e = ObjectIdentifier::decode(&[0x80, 0x01], mode).unwrap_err();
        assert_eq!(e, Error::new(ErrorClass::NonMinimalSubidentifier, 0));
        // 80 inside a subidentifier is not leading, and is legal: 81 80 01 = 16385.
        assert!(ObjectIdentifier::decode(&[0x2A, 0x81, 0x80, 0x01], mode).is_ok());
    }
}

#[test]
fn a_subidentifier_that_never_ends_is_bad_contents() {
    // constructed by hand from §8.19.2 ¶3: the last octet has bit 8 set.
    for mode in MODES {
        let e = ObjectIdentifier::decode(&[0x2A, 0x86, 0x48, 0x86], mode).unwrap_err();
        assert_eq!(e, Error::new(ErrorClass::BadContents, 3));
        assert_eq!(
            err_class(ObjectIdentifier::decode(&[0x2A, 0x81], mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(ObjectIdentifier::decode(&[0x88, 0x37, 0x03, 0xFF], mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(ObjectIdentifier::decode(&[0xFF], mode)),
            ErrorClass::BadContents
        );
    }
}

#[test]
fn empty_contents_are_refused() {
    // constructed by hand; derived from §8.19.3 and X.680 §32.11 (no first subidentifier).
    for mode in MODES {
        assert_eq!(
            ObjectIdentifier::decode(&[], mode).unwrap_err(),
            Error::new(ErrorClass::BadContents, 0)
        );
        assert_eq!(
            RelativeOid::decode(&[], mode).unwrap_err(),
            Error::new(ErrorClass::BadContents, 0)
        );
    }
}

// ---- RELATIVE-OID -------------------------------------------------------------------------------------

#[test]
fn relative_oid_example_8571_3_2() {
    // from X.690 §8.20.5: contents C2 7B 03 02 (8571 = 66*128 + 123).
    let content = [0xC2, 0x7B, 0x03, 0x02];
    let rel = RelativeOid::decode(&content, Mode::Ber).unwrap();
    assert_eq!(rel.contents(), content);
    assert_eq!(arcs_of(rel.arcs()), some(&[8571, 3, 2]));
    assert_eq!(enc_rel(&[8571, 3, 2]), content);
}

#[test]
fn relative_oid_has_no_first_two_arc_packing() {
    // constructed by hand from §8.20.3, §8.20.4: 0x2A is the single arc 42, not 1.2.
    let rel = RelativeOid::decode(&[0x2A], Mode::Der).unwrap();
    assert_eq!(arcs_of(rel.arcs()), some(&[42]));
    assert_eq!(enc_rel(&[42]), [0x2A]);
    assert_eq!(enc_rel(&[0]), [0x00]);
    // A single-arc value, and a first arc of 80 or more, are fine.
    assert_eq!(enc_rel(&[80]), [0x50]);
    assert_eq!(enc_rel(&[128]), [0x81, 0x00]);
}

#[test]
fn relative_oid_refusals() {
    for mode in MODES {
        // constructed by hand from §8.20.2 ¶6 and ¶3.
        assert_eq!(
            RelativeOid::decode(&[0x80, 0x01], mode).unwrap_err(),
            Error::new(ErrorClass::NonMinimalSubidentifier, 0)
        );
        assert_eq!(
            err_class(RelativeOid::decode(&[0x80], mode)),
            ErrorClass::NonMinimalSubidentifier
        );
        assert_eq!(
            RelativeOid::decode(&[0x03, 0x81], mode).unwrap_err(),
            Error::new(ErrorClass::BadContents, 1)
        );
    }
}

#[test]
fn relative_oid_large_arcs() {
    let bytes = enc_rel(&[1, u128::MAX]);
    let rel = RelativeOid::decode(&bytes, Mode::Ber).unwrap();
    assert_eq!(arcs_of(rel.arcs()), some(&[1, u128::MAX]));
}

// ---- encoders -----------------------------------------------------------------------------------------

#[test]
fn encoder_refusals() {
    let mut b = [0u8; 32];
    assert_eq!(
        encode_object_identifier_u64(&[], &mut b),
        Err(EncodeError::TooFewArcs)
    );
    assert_eq!(
        encode_object_identifier_u64(&[1], &mut b),
        Err(EncodeError::TooFewArcs)
    );
    assert_eq!(
        encode_object_identifier_u64(&[3, 0], &mut b),
        Err(EncodeError::FirstArcOutOfRange)
    );
    assert_eq!(
        encode_object_identifier_u64(&[1, 40], &mut b),
        Err(EncodeError::SecondArcOutOfRange)
    );
    assert_eq!(
        encode_object_identifier_u64(&[0, 40], &mut b),
        Err(EncodeError::SecondArcOutOfRange)
    );
    assert_eq!(encode_object_identifier_u64(&[1, 39], &mut b), Ok(1));
    assert_eq!(
        encode_relative_oid_u64(&[], &mut b),
        Err(EncodeError::TooFewArcs)
    );
}

#[test]
fn encoder_reports_a_short_buffer_at_every_length() {
    let arcs = [1u64, 2, 840, 113_549];
    let mut full = [0u8; 16];
    let n = encode_object_identifier_u64(&arcs, &mut full).unwrap();
    assert_eq!(&full[..n], [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D]);
    for len in 0..n {
        let mut small = vec![0u8; len];
        assert_eq!(
            encode_object_identifier_u64(&arcs, &mut small),
            Err(EncodeError::BufferTooSmall),
            "{len}"
        );
    }
    let mut exact = vec![0u8; n];
    assert_eq!(encode_object_identifier_u64(&arcs, &mut exact), Ok(n));
    // First arc 2 with a second arc of 48 needs two octets.
    assert_eq!(encode_object_identifier_u64(&[2, 47], &mut [0u8; 1]), Ok(1));
    assert_eq!(
        encode_object_identifier_u64(&[2, 48], &mut [0u8; 1]),
        Err(EncodeError::BufferTooSmall)
    );
}

#[test]
fn big_endian_arcs_ignore_leading_zeros_and_empty_is_zero() {
    let mut b = [0u8; 32];
    let arcs: [&[u8]; 3] = [&[0, 0, 2], &[], &[0, 0, 0x03, 0x48]];
    let n = encode_object_identifier_be(&arcs, &mut b).unwrap();
    // 2.0.840: first subidentifier 2*40 + 0 = 80 = 0x50, then 840 = 86 48.
    assert_eq!(&b[..n], [0x50, 0x86, 0x48]);
    let rel: [&[u8]; 2] = [&[0x01, 0x00, 0x00], &[0x00]];
    let n = encode_relative_oid_be(&rel, &mut b).unwrap();
    // 65536 = 4*16384 = 84 80 00.
    assert_eq!(&b[..n], [0x84, 0x80, 0x00, 0x00]);
    // A magnitude of 20 octets (more than a u128) round-trips through be_bytes.
    let huge: Vec<u8> = (1..=20).collect();
    let arcs: [&[u8]; 3] = [&[1], &[2], &huge];
    let n = encode_object_identifier_be(&arcs, &mut b).unwrap();
    let oid = ObjectIdentifier::decode(&b[..n], Mode::Der).unwrap();
    let arc = oid.arcs().nth(2).unwrap();
    assert_eq!(arc.to_u128(), None);
    let mut out = [0u8; 32];
    let m = arc.be_bytes(&mut out).unwrap();
    assert_eq!(&out[..m], &huge[..]);
}

#[test]
fn u64_and_u128_encoders_agree() {
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    let n = encode_object_identifier_u64(&[2, 999, 3, u64::MAX], &mut a).unwrap();
    let m = encode_object_identifier_u128(&[2, 999, 3, u128::from(u64::MAX)], &mut b).unwrap();
    assert_eq!(a[..n], b[..m]);
}

#[test]
fn encode_then_decode_at_every_group_boundary() {
    // Round-trips are weak alone (the literal vectors above pin the bytes) but cover every length boundary.
    for shift in 0..=127u32 {
        for delta in [0u128, 1] {
            for base in [(1u128 << shift).wrapping_sub(1), 1u128 << shift] {
                let v = base.wrapping_add(delta);
                let b = enc(&[1, 2, v]);
                let oid = ObjectIdentifier::decode(&b, Mode::Der).unwrap();
                assert_eq!(arcs_of(oid.arcs()), some(&[1, 2, v]), "{v}");
                let c = enc(&[2, v]);
                let oid = ObjectIdentifier::decode(&c, Mode::Der).unwrap();
                assert_eq!(arcs_of(oid.arcs()), some(&[2, v]), "2.{v}");
            }
        }
    }
}

// ---- OID-IRI and RELATIVE-OID-IRI ---------------------------------------------------------------------

#[test]
fn oid_iri_contents() {
    // constructed by hand from §8.21.2 ¶1: the UTF-8 of "/2/999" is 2F 32 2F 39 39 39. The identifier octets
    // (1F 23) belong to the framing module.
    for mode in MODES {
        let iri = OidIri::decode(&[0x2F, 0x32, 0x2F, 0x39, 0x39, 0x39], mode).unwrap();
        assert_eq!(iri.as_str(), "/2/999");
        assert_eq!(iri.arcs().collect::<Vec<_>>(), ["2", "999"]);
    }
    let mut b = [0u8; 16];
    let n = OidIri::encode("/2/999", &mut b).unwrap();
    assert_eq!(&b[..n], [0x2F, 0x32, 0x2F, 0x39, 0x39, 0x39]);
    // Non-integer labels are accepted as text; their character rules (X.660) are unread.
    let iri = OidIri::decode(b"/ASN.1/Basic-Encoding", Mode::Ber).unwrap();
    assert_eq!(iri.arcs().collect::<Vec<_>>(), ["ASN.1", "Basic-Encoding"]);
}

#[test]
fn relative_oid_iri_contents() {
    // constructed by hand from §8.22.2 ¶1 and X.680 §35.3: the first arc has no slash.
    for mode in MODES {
        let iri = RelativeOidIri::decode(&[0x34, 0x2F, 0x33], mode).unwrap();
        assert_eq!(iri.as_str(), "4/3");
        assert_eq!(iri.arcs().collect::<Vec<_>>(), ["4", "3"]);
    }
    let mut b = [0u8; 16];
    let n = RelativeOidIri::encode("4/3", &mut b).unwrap();
    assert_eq!(&b[..n], [0x34, 0x2F, 0x33]);
}

#[test]
fn iri_refusals() {
    for mode in MODES {
        // overlong "/" (C0 AF), from §8.21.2 ¶2.
        assert_eq!(
            OidIri::decode(&[0x2F, 0x31, 0xC0, 0xAF], mode).unwrap_err(),
            Error::new(ErrorClass::BadContents, 2)
        );
        assert_eq!(
            err_class(OidIri::decode(&[0xC0, 0xAF], mode)),
            ErrorClass::BadContents
        );
        // empty contents, no slash, empty label, trailing slash, white-space, integer label with leading zero.
        assert_eq!(
            err_class(OidIri::decode(&[], mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(OidIri::decode(b"2/999", mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(OidIri::decode(b"/", mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(OidIri::decode(b"//2", mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(OidIri::decode(b"/2/", mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(OidIri::decode(b"/2 /3", mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(OidIri::decode(b"/2/03", mode)),
            ErrorClass::BadContents
        );
        assert!(OidIri::decode(b"/2/0", mode).is_ok());
        // relative: a leading slash would be an empty first label.
        assert_eq!(
            err_class(RelativeOidIri::decode(b"/4/3", mode)),
            ErrorClass::BadContents
        );
        assert_eq!(
            err_class(RelativeOidIri::decode(&[], mode)),
            ErrorClass::BadContents
        );
        // an encoded surrogate (ED A0 80) is not UTF-8.
        assert_eq!(
            err_class(RelativeOidIri::decode(&[0x34, 0xED, 0xA0, 0x80], mode)),
            ErrorClass::BadContents
        );
    }
    let mut b = [0u8; 2];
    assert_eq!(
        OidIri::encode("2/999", &mut b),
        Err(EncodeError::InvalidIri)
    );
    assert_eq!(
        OidIri::encode("/2/999", &mut b),
        Err(EncodeError::BufferTooSmall)
    );
}
