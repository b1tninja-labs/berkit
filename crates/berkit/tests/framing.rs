//! Framing-layer tests over the public API. Vectors are labelled `from X.690 <place>` or `constructed by hand`
//! (from the clause named); every one was recomputed here and the corrections to `docs/design/framing.md`
//! are listed in `crates/berkit/roadmap/framing.md`.

use berkit::length::{self, Length};
use berkit::tag::{Class, Tag};
use berkit::tlv::{self, Limits, Tlv, WriteError};
use berkit::{ErrorClass as E, Mode};

/// Hex with optional spaces.
fn h(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// A leaked slice, so a parsed element can outlive the expression that built its input.
fn hl(s: &str) -> &'static [u8] {
    Box::leak(h(s).into_boxed_slice())
}

fn rep(prefix: &str, byte: u8, n: usize, suffix: &str) -> Vec<u8> {
    let mut v = h(prefix);
    v.extend(std::iter::repeat_n(byte, n));
    v.extend(h(suffix));
    v
}

const BER: Mode = Mode::Ber;
const DER: Mode = Mode::Der;

fn parse(input: &[u8], mode: Mode) -> Result<(Tlv<'_>, &[u8]), berkit::Error> {
    Tlv::parse(input, mode, &Limits::DEFAULT)
}

fn exact(input: &[u8], mode: Mode) -> Result<Tlv<'_>, berkit::Error> {
    Tlv::parse_exact(input, mode, &Limits::DEFAULT)
}

fn class_of(r: Result<Tlv<'_>, berkit::Error>) -> E {
    r.unwrap_err().class
}

// ---------------------------------------------------------------- tags

#[test]
fn tag_vectors_decode_and_encode_to_the_literal_octets() {
    use Class::{Application as A, ContextSpecific as C, Private as P, Universal as U};
    // (hex of the identifier octets, class, constructed, number). Every row: constructed by hand from
    // X.690 §8.1.2.2 (Table 1, bit 6, numbers 0..30) or §8.1.2.4.2 (base-128 groups).
    let rows: &[(&str, Class, bool, u64)] = &[
        ("02", U, false, 2),
        ("30", U, true, 16),
        ("40", A, false, 0),
        ("80", C, false, 0),
        ("C0", P, false, 0),
        ("A0", C, true, 0),
        ("9E", C, false, 30),
        ("9F 1F", C, false, 31),
        ("9F 7F", C, false, 127),
        ("9F 81 00", C, false, 128),
        ("9F FF 7F", C, false, 16383),
        ("9F 81 80 00", C, false, 16384),
        ("BF 1F", C, true, 31),
        ("5F 1F", A, false, 31),
        ("FF 1F", P, true, 31),
        ("1F 1F", U, false, 31),
        ("9F 90 80 80 80 00", C, false, 4_294_967_296),
        ("9F 81 FF FF FF FF FF FF FF FF 7F", C, false, u64::MAX),
    ];
    for &(hex, class, constructed, number) in rows {
        let bytes = h(hex);
        let (tag, used) = Tag::decode(&bytes).unwrap();
        assert_eq!(tag, Tag::new(class, constructed, number), "{hex}");
        assert_eq!(used, bytes.len(), "{hex}");
        assert_eq!(tag.encoded_len(), bytes.len(), "{hex}");
        let mut out = [0u8; 16];
        let n = tag.encode(&mut out).unwrap();
        assert_eq!(&out[..n], &bytes[..], "encode {hex}");
    }
}

#[test]
fn tag_decode_stops_after_its_own_octets() {
    let (tag, used) = Tag::decode(&h("9F 81 00 05 00")).unwrap();
    assert_eq!((tag.number, used), (128, 3));
}

#[test]
fn tag_errors() {
    let cls = |s: &str| Tag::decode(&h(s)).unwrap_err().class;
    // constructed by hand from §8.1.2.4.2 c): first subsequent octet has bits 7-1 all zero.
    assert_eq!(cls("1F 00 00"), E::NonMinimalTag);
    assert_eq!(cls("9F 80 01 00"), E::NonMinimalTag);
    assert_eq!(cls("9F 80 80 7F 00"), E::NonMinimalTag);
    // constructed by hand from §8.1.2.2 with §8.1.2.4: number 30 in the high form (derived rule).
    assert_eq!(cls("9F 1E 00"), E::NonMinimalTag);
    assert_eq!(cls("9F 01"), E::NonMinimalTag);
    // 31 is the smallest number that may use the high form.
    assert!(Tag::decode(&h("9F 1F")).is_ok());
    // §8.1.2.4.2 a): bit 8 set, input ends.
    assert_eq!(cls("9F 81"), E::Truncated);
    assert_eq!(cls("9F"), E::Truncated);
    assert_eq!(cls(""), E::Truncated);
    // 2^64: 82 then nine 7-bit zero groups. The notes' vector `9F 82 80x8 00 00` is right (10 subsequent
    // octets, then the length octet 00).
    assert_eq!(cls("9F 82 80 80 80 80 80 80 80 80 00 00"), E::LimitExceeded);
    // 2^70 style: more octets still overflow.
    assert_eq!(cls("9F 81 80 80 80 80 80 80 80 80 80 00"), E::LimitExceeded);
}

#[test]
fn tag_class_bits_round_trip() {
    for b in 0..4 {
        assert_eq!(Class::from_bits(b).bits(), b);
    }
}

#[test]
fn tag_encoded_len_boundaries() {
    let len = |n: u64| Tag::new(Class::Private, false, n).encoded_len();
    assert_eq!(
        [len(30), len(31), len(127), len(128), len(16383), len(16384)],
        [1, 2, 2, 3, 3, 4]
    );
    assert_eq!(len(u64::MAX), 11);
    assert_eq!(len(1 << 32), 6);
}

#[test]
fn tag_encode_buffer_too_small() {
    let tag = Tag::new(Class::Private, false, 16384);
    let mut out = [0u8; 3];
    assert_eq!(tag.encode(&mut out).unwrap_err().needed, 4);
}

// ---------------------------------------------------------------- lengths

#[test]
fn length_vectors_ber_and_der() {
    // constructed by hand from §8.1.3.4 (short) and §8.1.3.5 (long); 201 = `81 C9` is from the §8.1.3.5 EXAMPLE.
    let rows: &[(&str, u64)] = &[
        ("00", 0),
        ("7F", 127),
        ("81 80", 128),
        ("81 C9", 201),
        ("81 FF", 255),
        ("82 01 00", 256),
        ("82 FF FF", 65535),
        ("83 01 00 00", 65536),
        ("88 FF FF FF FF FF FF FF FF", u64::MAX),
    ];
    for &(hex, value) in rows {
        let bytes = h(hex);
        for mode in [BER, DER] {
            assert_eq!(
                length::decode(&bytes, mode).unwrap(),
                (Length::Definite(value), bytes.len()),
                "{hex}"
            );
        }
        let mut out = [0u8; 16];
        let n = Length::Definite(value).encode(&mut out).unwrap();
        assert_eq!(&out[..n], &bytes[..], "encode {hex}");
        assert_eq!(Length::Definite(value).encoded_len(), n);
    }
}

#[test]
fn length_indefinite_is_a_single_80_and_81_00_is_zero() {
    assert_eq!(
        length::decode(&h("80"), BER).unwrap(),
        (Length::Indefinite, 1)
    );
    assert_eq!(
        length::decode(&h("81 00"), BER).unwrap(),
        (Length::Definite(0), 2)
    );
    assert_eq!(
        length::decode(&h("80"), DER).unwrap_err().class,
        E::IndefiniteLength
    );
    let mut out = [0u8; 1];
    assert_eq!(Length::Indefinite.encode(&mut out), Ok(1));
    assert_eq!(out, [0x80]);
}

#[test]
fn length_non_minimal_long_forms_ber_accepts_der_rejects() {
    // constructed by hand from §8.1.3.5 NOTE 2 (BER) and §10.1 (DER).
    let rows: &[(&str, u64)] = &[
        ("81 00", 0),
        ("81 01", 1),
        ("81 7F", 127),
        ("82 00 80", 128),
        ("82 00 01", 1),
        ("84 00 00 00 01", 1),
        ("89 00 00 00 00 00 00 00 00 01", 1),
        ("89 00 FF FF FF FF FF FF FF FF", u64::MAX),
    ];
    for &(hex, value) in rows {
        let bytes = h(hex);
        assert_eq!(
            length::decode(&bytes, BER).unwrap(),
            (Length::Definite(value), bytes.len()),
            "{hex}"
        );
        assert_eq!(
            length::decode(&bytes, DER).unwrap_err().class,
            E::NonMinimalLength,
            "{hex}"
        );
        assert!(!length::decode_raw(&bytes).unwrap().minimal);
    }
    // §8.1.3.5 b): 0xFE announces 126 length octets, the largest count left by c). All leading zeros, value 1.
    let mut v = vec![0xFE];
    v.extend([0u8; 125]);
    v.push(1);
    assert_eq!(length::decode(&v, BER).unwrap(), (Length::Definite(1), 127));
    assert_eq!(
        length::decode(&v, DER).unwrap_err().class,
        E::NonMinimalLength
    );
}

#[test]
fn length_errors() {
    let cls = |s: &str, m| length::decode(&h(s), m).unwrap_err().class;
    // constructed by hand from §8.1.3.5 c).
    assert_eq!(cls("FF", BER), E::ReservedLength);
    assert_eq!(cls("FF", DER), E::ReservedLength);
    // reserved wins over truncation: the initial octet alone decides it.
    assert_eq!(cls("FF 01", BER), E::ReservedLength);
    // value 2^64 in nine octets, and a nonzero octet beyond eight.
    assert_eq!(cls("89 01 00 00 00 00 00 00 00 00", BER), E::LimitExceeded);
    assert_eq!(cls("89 01 00 00 00 00 00 00 00 00", DER), E::LimitExceeded);
    assert_eq!(
        cls("8A 00 01 00 00 00 00 00 00 00 00", BER),
        E::LimitExceeded
    );
    // truncation of the length octets.
    assert_eq!(cls("81", BER), E::Truncated);
    assert_eq!(cls("82 01", BER), E::Truncated);
    assert_eq!(cls("", BER), E::Truncated);
    // a value that cannot be decided is still truncated when the octets are missing.
    assert_eq!(cls("89 01 00", BER), E::Truncated);
}

#[test]
fn length_encode_buffer_too_small() {
    let mut out = [0u8; 2];
    assert_eq!(
        Length::Definite(65536).encode(&mut out).unwrap_err().needed,
        4
    );
}

// ---------------------------------------------------------------- tlv: well-formed

/// (hex, class, constructed, number, definite length or None for indefinite), BER and DER alike.
#[test]
fn tlv_vectors_ok_in_both_modes() {
    use Class::{Application as A, ContextSpecific as C, Universal as U};
    let rows: Vec<(Vec<u8>, Class, bool, u64, u64)> = vec![
        // from X.690 §8.2.2 EXAMPLE (BOOLEAN TRUE), §8.8.2 EXAMPLE (NULL).
        (h("01 01 FF"), U, false, 1, 1),
        (h("05 00"), U, false, 5, 0),
        // from X.690 §8.9.3 EXAMPLE (SEQUENCE).
        (h("30 0A 16 05 53 6D 69 74 68 01 01 FF"), U, true, 16, 10),
        // from X.690 §8.14.4 EXAMPLE (Type2, Type3, Type4, Type5).
        (h("43 05 4A 6F 6E 65 73"), A, false, 3, 5),
        (h("A2 07 43 05 4A 6F 6E 65 73"), C, true, 2, 7),
        (h("67 07 43 05 4A 6F 6E 65 73"), A, true, 7, 7),
        (h("82 05 4A 6F 6E 65 73"), C, false, 2, 5),
        // from X.690 §8.6.4.2 EXAMPLE (primitive BIT STRING).
        (h("03 07 04 0A 3B 5F 29 1C D0"), U, false, 3, 7),
        // from X.690 §8.23.6 example, primitive and constructed definite.
        (h("1A 05 4A 6F 6E 65 73"), U, false, 26, 5),
        (h("3A 09 04 03 4A 6F 6E 04 02 65 73"), U, true, 26, 9),
        // constructed by hand from §8.1.3.4: length 0.
        (h("04 00"), U, false, 4, 0),
        (h("30 00"), U, true, 16, 0),
        (h("9F 1F 00"), C, false, 31, 0),
        (h("30 04 30 02 05 00"), U, true, 16, 4),
        // length 38: from X.690 §8.1.3.4 EXAMPLE (0x26).
        (rep("04 26", 0x41, 38, ""), U, false, 4, 38),
        (rep("04 7F", 0, 127, ""), U, false, 4, 127),
        (rep("04 81 80", 0, 128, ""), U, false, 4, 128),
        (rep("04 81 C9", 0, 201, ""), U, false, 4, 201),
        (rep("04 81 FF", 0, 255, ""), U, false, 4, 255),
        (rep("04 82 01 00", 0, 256, ""), U, false, 4, 256),
        (rep("04 82 FF FF", 0, 65535, ""), U, false, 4, 65535),
        (rep("04 83 01 00 00", 0, 65536, ""), U, false, 4, 65536),
    ];
    for (bytes, class, constructed, number, len) in rows {
        for mode in [BER, DER] {
            let t = exact(&bytes, mode).unwrap();
            assert_eq!(t.tag(), Tag::new(class, constructed, number));
            assert_eq!(t.length(), Length::Definite(len));
            assert_eq!(t.content().len() as u64, len);
            assert_eq!(t.encoding(), &bytes[..]);
            assert_eq!([t.header(), t.content()].concat(), bytes);
            assert_eq!(t.offset(), 0);
        }
    }
}

#[test]
fn tlv_slices_are_the_exact_input_octets() {
    // constructed by hand: a non-minimal length and a high tag, so a re-encoding would differ.
    let input = h("9F 81 00 82 00 02 AA BB CC");
    let (t, rest) = parse(&input, BER).unwrap();
    assert_eq!(t.header(), &input[..6]);
    assert_eq!(t.content(), &[0xAA, 0xBB]);
    assert_eq!(t.encoding(), &input[..8]);
    assert_eq!(rest, &[0xCC]);
    assert!(std::ptr::eq(t.header().as_ptr(), input.as_ptr()));
    assert!(std::ptr::eq(t.content().as_ptr(), input[6..].as_ptr()));
}

#[test]
fn tlv_non_minimal_length_is_kept_and_der_rejects_it() {
    for (hex, content_len) in [
        ("04 81 01 41", 1),
        ("04 81 00", 0),
        ("04 82 00 01 41", 1),
        ("04 84 00 00 00 01 41", 1),
        ("04 89 00 00 00 00 00 00 00 00 01 41", 1),
    ] {
        let b = h(hex);
        let t = exact(&b, BER).unwrap();
        assert_eq!(t.content().len(), content_len, "{hex}");
        assert_eq!(class_of(exact(&b, DER)), E::NonMinimalLength, "{hex}");
    }
    let b = rep("04 81 7F", 0, 127, "");
    assert!(exact(&b, BER).is_ok());
    assert_eq!(class_of(exact(&b, DER)), E::NonMinimalLength);
    let b = rep("04 82 00 80", 0, 128, "");
    assert!(exact(&b, BER).is_ok());
    assert_eq!(class_of(exact(&b, DER)), E::NonMinimalLength);
    let mut b = vec![0x04, 0xFE];
    b.extend([0u8; 125]);
    b.extend([1, 0x41]);
    assert!(exact(&b, BER).is_ok());
    assert_eq!(class_of(exact(&b, DER)), E::NonMinimalLength);
}

#[test]
fn tlv_unassigned_universal_numbers_pass_at_framing() {
    // X.680 Table 1 reserves 15 and 37+; the typed layer refuses them, framing does not (policy).
    assert!(exact(&h("0F 00"), BER).is_ok());
    assert!(exact(&h("1F 25 00"), BER).is_ok());
}

// ---------------------------------------------------------------- tlv: indefinite and EOC

#[test]
fn indefinite_forms_ber() {
    // constructed by hand from §8.1.3.6 and §8.1.5.
    let t = exact(hl("30 80 00 00"), BER).unwrap();
    assert_eq!(
        (t.length(), t.content().len(), t.encoding().len()),
        (Length::Indefinite, 0, 4)
    );
    let t = exact(hl("30 80 02 01 05 00 00"), BER).unwrap();
    assert_eq!(t.content(), &h("02 01 05")[..]);
    assert_eq!(t.encoding().len(), 7);
    assert!(t.is_indefinite());
    // nested: each indefinite encoding has its own EOC; the first closes the inner.
    let t = exact(hl("30 80 30 80 00 00 00 00"), BER).unwrap();
    assert_eq!(t.content(), &h("30 80 00 00")[..]);
    let inner = t.children().next().unwrap().unwrap();
    assert_eq!(inner.encoding(), &h("30 80 00 00")[..]);
    assert_eq!(inner.offset(), 2);
    // definite parent holding an indefinite child: the parent's length counts the child's EOC.
    let t = exact(hl("30 04 30 80 00 00"), BER).unwrap();
    assert_eq!(t.children().count(), 1);
    // outer indefinite, inner definite: the single 00 00 closes the outer (X.690 §8.1.1.2).
    let t = exact(hl("30 80 30 03 02 01 05 00 00"), BER).unwrap();
    assert_eq!(t.content(), &h("30 03 02 01 05")[..]);
    // from X.690 §8.6.4.2 EXAMPLE (constructed BIT STRING) and §8.23.6 (constructed VisibleString).
    let t = exact(hl("23 80 03 03 00 0A 3B 03 05 04 5F 29 1C D0 00 00"), BER).unwrap();
    assert_eq!(t.children().count(), 2);
    let t = exact(hl("3A 80 04 03 4A 6F 6E 04 02 65 73 00 00"), BER).unwrap();
    assert_eq!(t.children().count(), 2);
    // content octets that look like an EOC are not one: the EOC is only read at an element boundary.
    let t = exact(hl("24 80 04 02 00 00 00 00"), BER).unwrap();
    assert_eq!(t.children().next().unwrap().unwrap().content(), &[0, 0]);
}

#[test]
fn indefinite_der_is_refused_everywhere_by_header() {
    for hex in [
        "30 80 00 00",
        "30 80 02 01 05 00 00",
        "30 80 30 80 00 00 00 00",
        "30 80 30 80 00 00",
        "30 80 02 01 05",
        "30 80 02 01 05 00",
        "30 80 00 01 05",
        "30 04 30 80 00 00",
        "30 03 30 80 00",
        "23 80 03 03 00 0A 3B 03 05 04 5F 29 1C D0 00 00",
        "30 80",
    ] {
        assert_eq!(class_of(exact(&h(hex), DER)), E::IndefiniteLength, "{hex}");
    }
}

#[test]
fn indefinite_on_primitive_is_decided_by_the_identifier() {
    for hex in ["04 80 41 00 00", "02 80 05 00 00", "04 80", "02 80"] {
        for mode in [BER, DER] {
            assert_eq!(
                class_of(exact(&h(hex), mode)),
                E::IndefiniteOnPrimitive,
                "{hex}"
            );
        }
    }
}

#[test]
fn eoc_errors() {
    // missing EOC (corpus/format.md: the contents end at a boundary without 00 00). The notes say
    // `truncated`; the shared class list has `missing_eoc` for exactly this.
    for hex in [
        "30 80 02 01 05",
        "30 80 30 80 00 00",
        "30 80",
        "30 80 30 03 02 01 05",
    ] {
        assert_eq!(class_of(exact(&h(hex), BER)), E::MissingEoc, "{hex}");
    }
    // a half EOC: the identifier of a next element with no length octet.
    assert_eq!(class_of(exact(&h("30 80 02 01 05 00"), BER)), E::Truncated);
    // inside a definite parent the same cut is the parent's end.
    assert_eq!(class_of(exact(&h("30 03 30 80 00"), BER)), E::LengthOverrun);
    assert_eq!(class_of(exact(&h("30 02 30 80"), BER)), E::MissingEoc);
    // misplaced EOC: top level, inside a definite parent, tag 0 that is not `00 00`, constructed tag 0.
    for hex in [
        "00 00",
        "30 02 00 00",
        "00 01 05",
        "20 00",
        "30 80 00 01 05",
        "30 80 20 00",
    ] {
        assert_eq!(class_of(exact(&h(hex), BER)), tlv::STRAY_EOC, "{hex}");
        assert_eq!(
            class_of(exact(&h(hex), DER)),
            if hex.starts_with("30 80") {
                E::IndefiniteLength
            } else {
                tlv::STRAY_EOC
            },
            "{hex}"
        );
    }
    // `00 81 00` is not the two octets 00 00.
    assert_eq!(class_of(exact(&h("30 80 00 81 00"), BER)), tlv::STRAY_EOC);
    // tag number 0 outside the universal class is an ordinary tag.
    assert!(exact(&h("80 00"), BER).is_ok());
    // an EOC after a complete definite value belongs to no encoding: trailing data.
    let e = exact(&h("30 00 00 00"), BER).unwrap_err();
    assert_eq!((e.class, e.offset), (E::TrailingData, 2));
    let (t, rest) = parse(hl("30 00 00 00"), BER).unwrap();
    assert_eq!((t.encoding().len(), rest), (2, &[0u8, 0][..]));
}

// ---------------------------------------------------------------- tlv: faults, stream order

#[test]
fn header_and_extent_errors() {
    let cls = |hex: &str, mode| class_of(exact(&h(hex), mode));
    for mode in [BER, DER] {
        assert_eq!(cls("04 FF 01 41", mode), E::ReservedLength);
        assert_eq!(cls("04 81", mode), E::Truncated);
        assert_eq!(cls("04 82 01", mode), E::Truncated);
        assert_eq!(cls("04 05 41 42", mode), E::Truncated);
        assert_eq!(cls("04", mode), E::Truncated);
        assert_eq!(cls("", mode), E::Truncated);
        assert_eq!(cls("9F 81", mode), E::Truncated);
        assert_eq!(cls("04 88 FF FF FF FF FF FF FF FF", mode), E::Truncated);
        assert_eq!(
            cls("04 89 01 00 00 00 00 00 00 00 00", mode),
            E::LimitExceeded
        );
        assert_eq!(cls("1F 00 00", mode), E::NonMinimalTag);
        assert_eq!(cls("9F 1E 00", mode), E::NonMinimalTag);
        assert_eq!(cls("9F 80 01 00", mode), E::NonMinimalTag);
        assert_eq!(
            cls("9F 82 80 80 80 80 80 80 80 80 00 00", mode),
            E::LimitExceeded
        );
        // child overruns parent (constructed by hand from §8.9.2, §8.1.4).
        assert_eq!(cls("30 03 02 02 05", mode), E::LengthOverrun);
        assert_eq!(cls("30 02 02 01 05", mode), E::LengthOverrun);
        assert_eq!(cls("30 05 02 01 05 04 01 41", mode), E::LengthOverrun);
        // the child's header, not only its contents, can overrun.
        assert_eq!(cls("30 01 02", mode), E::LengthOverrun);
        assert_eq!(cls("30 02 02 82", mode), E::LengthOverrun);
    }
    // accepted tag numbers
    assert!(exact(&h("9F 90 80 80 80 00 00"), BER).is_ok());
    assert!(exact(&h("9F 81 FF FF FF FF FF FF FF FF 7F 00"), BER).is_ok());
}

#[test]
fn stream_order_reports_the_first_fault() {
    // header fault of an element comes before a fault in its contents.
    assert_eq!(class_of(exact(&h("30 02 9F 1E"), BER)), E::NonMinimalTag);
    // a child's header fault comes before a later sibling's.
    assert_eq!(
        class_of(exact(&h("30 06 04 FF 00 02 82 05"), BER)),
        E::ReservedLength
    );
    // depth first: a fault deep in the first child beats a fault in the second.
    assert_eq!(
        class_of(exact(&h("30 0A 30 04 02 05 00 00 04 80 00 00"), BER)),
        E::LengthOverrun
    );
    // inside an indefinite parent a definite child's inner fault comes before the missing EOC.
    assert_eq!(
        class_of(exact(&h("30 80 30 03 02 02 05"), BER)),
        E::LengthOverrun
    );
    // in DER the indefinite header is the first fault even though contents are broken.
    assert_eq!(class_of(exact(&h("30 80 02 05"), DER)), E::IndefiniteLength);
    // a fault inside the element precedes trailing data.
    assert_eq!(
        class_of(exact(&h("30 03 02 02 05 AA"), BER)),
        E::LengthOverrun
    );
    // the primitive check precedes the DER non-minimal check, and DER rules apply to descendants.
    assert_eq!(
        class_of(exact(&h("30 04 04 81 01 41"), DER)),
        E::NonMinimalLength
    );
    assert!(exact(&h("30 04 04 81 01 41"), BER).is_ok());
}

#[test]
fn trailing_data_via_parse_exact() {
    let e = exact(&h("02 01 05 00"), BER).unwrap_err();
    assert_eq!((e.class, e.offset), (E::TrailingData, 3));
    let (t, rest) = parse(hl("02 01 05 00"), BER).unwrap();
    assert_eq!((t.content(), rest), (&[5u8][..], &[0u8][..]));
}

#[test]
fn children_iterate_without_allocation_and_keep_offsets() {
    let input = h("30 0A 16 05 53 6D 69 74 68 01 01 FF");
    let t = exact(&input, BER).unwrap();
    let kids: Vec<_> = t.children().map(Result::unwrap).collect();
    assert_eq!(kids.len(), 2);
    assert_eq!((kids[0].offset(), kids[0].encoding()), (2, &input[2..9]));
    assert_eq!((kids[1].offset(), kids[1].encoding()), (9, &input[9..]));
    // a primitive has no nested encodings.
    assert_eq!(kids[0].children().count(), 0);
    // after an error the iterator ends.
    let bad = h("30 04 02 01 05 FF");
    let t = parse(&bad, BER).unwrap().0;
    let mut it = t.children();
    assert!(it.next().unwrap().is_ok());
    assert_eq!(it.next().unwrap().unwrap_err().class, E::LengthOverrun);
    assert!(it.next().is_none());
}

// ---------------------------------------------------------------- limits

fn nested(levels: usize) -> Vec<u8> {
    // `levels` definite SEQUENCEs around an empty one; all lengths short (< 128) so levels <= 40 here.
    let mut v = h("30 00");
    for _ in 1..levels {
        let mut w = vec![0x30, u8::try_from(v.len()).unwrap()];
        w.extend(&v);
        v = w;
    }
    v
}

#[test]
fn depth_limit_is_limit_exceeded_not_a_crash() {
    let lim = Limits {
        max_depth: 5,
        ..Limits::DEFAULT
    };
    assert!(Tlv::parse_exact(&nested(5), BER, &lim).is_ok());
    assert_eq!(
        Tlv::parse_exact(&nested(6), BER, &lim).unwrap_err().class,
        E::LimitExceeded
    );
    // indefinite nesting is bounded too: 100 000 openers must not recurse 100 000 deep.
    let mut deep = Vec::new();
    for _ in 0..100_000 {
        deep.extend([0x30, 0x80]);
    }
    assert_eq!(
        Tlv::parse(&deep, BER, &Limits::DEFAULT).unwrap_err().class,
        E::LimitExceeded
    );
    // a huge max_depth is clamped to the hard bound.
    let huge = Limits {
        max_depth: usize::MAX,
        ..Limits::DEFAULT
    };
    assert_eq!(
        Tlv::parse(&deep, BER, &huge).unwrap_err().class,
        E::LimitExceeded
    );
    // the default is 64.
    assert!(Tlv::parse_exact(&nested(40), BER, &Limits::default()).is_ok());
    assert_eq!(Limits::default().max_depth, 64);
}

#[test]
fn content_limit() {
    let lim = Limits {
        max_content_length: 3,
        ..Limits::DEFAULT
    };
    assert!(Tlv::parse_exact(&h("04 03 01 02 03"), BER, &lim).is_ok());
    assert_eq!(
        Tlv::parse_exact(&h("04 04 01 02 03 04"), BER, &lim)
            .unwrap_err()
            .class,
        E::LimitExceeded
    );
    // octets that are not there are reported as missing before the limit applies.
    assert_eq!(
        Tlv::parse_exact(&h("04 05 01"), BER, &lim)
            .unwrap_err()
            .class,
        E::Truncated
    );
}

// ---------------------------------------------------------------- truncation sweeps

fn sweep(full: &[u8], mode: Mode, allowed: &[E]) {
    assert!(exact(full, mode).is_ok());
    for k in 0..full.len() {
        let e = exact(&full[..k], mode).unwrap_err();
        assert!(allowed.contains(&e.class), "prefix {k}: {e}");
    }
}

#[test]
fn truncation_sweep_a_every_prefix_is_truncated() {
    // constructed by hand from §8.1.1.1: `30 06 02 01 05 04 01 41`.
    let a = h("30 06 02 01 05 04 01 41");
    for mode in [BER, DER] {
        sweep(&a, mode, &[E::Truncated]);
    }
}

#[test]
fn truncation_sweep_b_high_tag_and_long_length() {
    // constructed by hand from §8.1.2.4.2 and §8.1.3.5: `9F 81 00 82 01 00` + 256 octets, EVERY position.
    let b = rep("9F 81 00 82 01 00", 0, 256, "");
    for mode in [BER, DER] {
        sweep(&b, mode, &[E::Truncated]);
    }
}

#[test]
fn truncation_sweep_c_indefinite() {
    // constructed by hand from §8.1.3.6.2 and §8.1.5: `30 80 02 01 05 00 00`. DER stops at the header.
    let c = h("30 80 02 01 05 00 00");
    sweep(&c, BER, &[E::Truncated, E::MissingEoc]);
    // Prefixes 2 and 5 end exactly at an element boundary with the EOC still owed; the others end inside
    // an identifier, a length or declared contents.
    let expected = |k: usize| {
        if k == 2 || k == 5 {
            E::MissingEoc
        } else {
            E::Truncated
        }
    };

    for k in 0..c.len() {
        assert_eq!(class_of(exact(&c[..k], BER)), expected(k), "prefix {k}");
        let der = class_of(exact(&c[..k], DER));
        assert_eq!(
            der,
            if k < 2 {
                E::Truncated
            } else {
                E::IndefiniteLength
            },
            "prefix {k}"
        );
    }
}

#[test]
fn truncation_sweep_inside_a_definite_parent_is_length_overrun_when_the_parent_lies() {
    // `30 06 02 01 05 04 01 41` with the parent length kept but the contents cut: top-level truncated;
    // with the parent length shortened to fit, the child overruns.
    let full = h("30 08 02 01 05 04 03 41 42 43");
    sweep(&full, BER, &[E::Truncated]);
    for cut in 3..full.len() {
        let mut v = full[..cut].to_vec();
        v[1] = u8::try_from(cut - 2).unwrap();
        let r = exact(&v, BER);
        if cut == 5 {
            assert!(r.is_ok());
        } else {
            assert_eq!(class_of(r), E::LengthOverrun, "cut {cut}");
        }
    }
}

#[test]
fn annex_a_record_parses_and_every_prefix_is_refused() {
    // from X.690 Annex A.3, rebuilt in docs/design/framing.md §4 (PersonnelRecord, 133 content octets).
    let rec = h(
        "60 81 85 61 10 1A 04 4A 6F 68 6E 1A 01 50 1A 05 53 6D 69 74 68 A0 0A 1A 08 44 69 72 65 63 74 6F 72 \
        42 01 33 A1 0A 43 08 31 39 37 31 30 39 31 37 A2 12 61 10 1A 04 4D 61 72 79 1A 01 54 1A 05 53 6D 69 74 68 \
        A3 42 31 1F 61 11 1A 05 52 61 6C 70 68 1A 01 54 1A 05 53 6D 69 74 68 A0 0A 43 08 31 39 35 37 31 31 31 31 \
        31 1F 61 11 1A 05 53 75 73 61 6E 1A 01 42 1A 05 4A 6F 6E 65 73 A0 0A 43 08 31 39 35 39 30 37 31 37",
    );
    for mode in [BER, DER] {
        let t = exact(&rec, mode).unwrap();
        assert_eq!(t.tag(), Tag::new(Class::Application, true, 0));
        assert_eq!(t.length(), Length::Definite(133));
        assert_eq!(t.header(), &rec[..3]);
        assert_eq!(t.children().count(), 6);
        sweep(&rec, mode, &[E::Truncated]);
    }
}

// ---------------------------------------------------------------- writer

#[test]
fn writer_emits_literal_wire_bytes() {
    let mut out = vec![0u8; 70_000];
    let u = |n, c| Tag::new(Class::Universal, c, n);
    // from X.690 §8.2.2 and §8.8.2 EXAMPLEs.
    let n = tlv::write_tlv(&u(1, false), &[0xFF], &mut out).unwrap();
    assert_eq!(&out[..n], &h("01 01 FF")[..]);
    let n = tlv::write_tlv(&u(5, false), &[], &mut out).unwrap();
    assert_eq!(&out[..n], &h("05 00")[..]);
    // from X.690 §8.14.4 EXAMPLE (Type3): [2] constructed around the Type2 encoding.
    let n = tlv::write_tlv(&tlv::context(2, true), &h("43 05 4A 6F 6E 65 73"), &mut out).unwrap();
    assert_eq!(&out[..n], &h("A2 07 43 05 4A 6F 6E 65 73")[..]);
    // constructed by hand: high tag and long length, minimal form.
    let n = tlv::write_tlv(&tlv::context(16384, false), &[1; 128], &mut out).unwrap();
    assert_eq!(&out[..7], &h("9F 81 80 00 81 80 01")[..]);
    assert_eq!(n, 4 + 2 + 128);
    assert_eq!(tlv::encoded_len(&tlv::context(16384, false), 128), n);
    let n = tlv::write_tlv(&u(4, false), &vec![0; 65536], &mut out).unwrap();
    assert_eq!(&out[..5], &h("04 83 01 00 00")[..]);
    assert_eq!(n, 5 + 65536);
    // headers: indefinite and EOC.
    let n = tlv::write_header(&u(16, true), Length::Indefinite, &mut out).unwrap();
    assert_eq!(&out[..n], &h("30 80")[..]);
    assert_eq!(tlv::write_eoc(&mut out), Ok(2));
    assert_eq!(&out[..2], &[0, 0]);
    let n = tlv::write_header(&u(4, false), Length::Definite(300), &mut out).unwrap();
    assert_eq!(&out[..n], &h("04 82 01 2C")[..]);
}

#[test]
fn writer_refusals() {
    let mut out = [0u8; 4];
    let u = |n, c| Tag::new(Class::Universal, c, n);
    assert_eq!(
        tlv::write_tlv(&u(0, false), &[], &mut out),
        Err(WriteError::ReservedTag)
    );
    assert_eq!(
        tlv::write_header(&u(4, false), Length::Indefinite, &mut out),
        Err(WriteError::IndefiniteOnPrimitive)
    );
    // too small: nothing is written and `needed` is the whole encoding.
    let mut small = [0xAAu8; 3];
    let e = tlv::write_tlv(&u(4, false), &[1, 2, 3], &mut small).unwrap_err();
    assert_eq!(
        e,
        WriteError::BufferTooSmall(berkit::tag::BufferTooSmall { needed: 5 })
    );
    assert_eq!(small, [0xAA; 3]);
    assert!(tlv::write_eoc(&mut small[..1]).is_err());
    assert!(tlv::write_header(&u(4, false), Length::Definite(1), &mut small[..1]).is_err());
}

#[test]
fn round_trip_writer_to_reader_with_literal_check() {
    let mut buf = vec![0u8; 70_000];
    let tags = [
        Tag::new(Class::Universal, false, 4),
        Tag::new(Class::Application, true, 30),
        Tag::new(Class::ContextSpecific, false, 31),
        Tag::new(Class::Private, true, 127),
        Tag::new(Class::ContextSpecific, false, 128),
        Tag::new(Class::Private, false, 16383),
        Tag::new(Class::Application, false, 16384),
        Tag::new(Class::Private, true, 1 << 32),
        Tag::new(Class::Private, false, u64::MAX),
    ];
    let lens = [0usize, 1, 127, 128, 255, 256, 65535];
    for tag in tags {
        for len in lens {
            let content = vec![0x5A; len];
            let n = tlv::write_tlv(&tag, &content, &mut buf).unwrap();
            for mode in [BER, DER] {
                let t = Tlv::parse(&buf[..n], mode, &Limits::DEFAULT).unwrap().0;
                assert_eq!(t.tag(), tag);
                assert_eq!(t.content(), &content[..]);
                assert_eq!(t.encoding().len(), n);
            }
            // header size is the sum of the minimal tag and length forms.
            let expect_len = tag.encoded_len()
                + if len < 128 {
                    1
                } else if len < 256 {
                    2
                } else {
                    3
                };
            assert_eq!(n - len, expect_len);
        }
    }
}

#[test]
fn a_stray_end_of_contents_is_the_stray_eoc_class() {
    // constructed by hand: `00 00` at the top level is the end-of-contents octets with nothing open to close
    // (X.690 §8.1.5, X.680 Table 1: UNIVERSAL 0 is reserved for the encoding rules).
    assert_eq!(
        class_of(exact(&h("00 00"), BER)),
        berkit::ErrorClass::StrayEoc
    );
    assert_eq!(tlv::STRAY_EOC.name(), "stray_eoc");
}

#[test]
fn a_long_form_length_with_a_chosen_number_of_octets_is_written_exactly() {
    // constructed by hand from X.690 §8.1.3.5 and its NOTE 2: the initial octet is `80 | k`, then k octets, so a
    // sender may pad the length with leading zero octets. `81 00` is a long-form zero, not the indefinite form.
    let mut out = [0u8; 16];
    for (value, octets, expect) in [
        (1u64, 1usize, "81 01"),
        (1, 2, "82 00 01"),
        (0x1234, 2, "82 12 34"),
        (0, 1, "81 00"),
        (1, 9, "89 00 00 00 00 00 00 00 00 01"),
    ] {
        let n = length::encode_long(value, octets, &mut out).expect("fits");
        assert_eq!(
            &out[..n],
            h(expect).as_slice(),
            "{value} in {octets} octets"
        );
    }
}

#[test]
fn a_long_form_length_is_refused_when_it_cannot_be_written() {
    let mut out = [0u8; 4];
    assert_eq!(
        length::encode_long(1, 0, &mut out),
        Err(length::LongFormError::OctetCount)
    );
    assert_eq!(
        length::encode_long(1, 127, &mut out),
        Err(length::LongFormError::OctetCount)
    );
    assert_eq!(
        length::encode_long(256, 1, &mut out),
        Err(length::LongFormError::ValueTooLarge)
    );
    assert!(matches!(
        length::encode_long(1, 4, &mut out),
        Err(length::LongFormError::BufferTooSmall(_))
    ));
}

#[test]
fn a_header_with_a_padded_long_length_reads_back_in_ber_and_is_refused_in_der() {
    // constructed by hand: `04 81 01 41` is an OCTET STRING of one octet whose length is written in the long form.
    let mut head = [0u8; 8];
    let tag = Tag::new(Class::Universal, false, 4);
    let n = tlv::write_header_long(&tag, 1, 1, &mut head).expect("writes");
    assert_eq!(&head[..n], h("04 81 01").as_slice());
    let whole = h("04 81 01 41");
    let ber =
        exact(&whole, Mode::Ber).expect("BER accepts the sender's choice (X.690 §8.1.3.5 NOTE 2)");
    assert_eq!(ber.content(), [0x41]);
    assert_eq!(class_of(exact(&whole, Mode::Der)), E::NonMinimalLength);
    let zero_tag = Tag::new(Class::Universal, false, 0);
    assert_eq!(
        tlv::write_header_long(&zero_tag, 1, 1, &mut head),
        Err(WriteError::ReservedTag)
    );
}

#[test]
fn the_padding_octets_of_a_long_form_length_are_written_into_a_dirty_buffer() {
    // constructed by hand: the writer must not rely on the caller's buffer being zeroed, so pre-fill it with FF.
    // Nine length octets is where the padding matters: a u64 fills only the last eight (X.690 §8.1.3.5 NOTE 2).
    let mut out = [0xFFu8; 12];
    let n = length::encode_long(1, 9, &mut out).expect("fits");
    assert_eq!(&out[..n], h("89 00 00 00 00 00 00 00 00 01").as_slice());
    assert_eq!(out[n], 0xFF, "nothing past the length octets is touched");
}
