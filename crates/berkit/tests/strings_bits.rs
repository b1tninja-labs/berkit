//! BIT STRING and OCTET STRING contents, over the public API. Vector ids V-Sxx and V-Oxx are those of
//! docs/design/primitives.md section 2. The tests take CONTENTS octets: `03 02 07 80` is the contents `07 80`.
//! Constructed vectors are given as the list of segment contents the TLV layer would supply.

use berkit::bitstring::{self, BitString, ConstructedBitString};
use berkit::octetstring::{self, ConstructedOctetString};
use berkit::{ErrorClass, Mode};

const BER: Mode = Mode::Ber;
const DER: Mode = Mode::Der;

fn bits_of(v: &BitString<'_>) -> Vec<bool> {
    v.bits().collect()
}

fn bin(s: &str) -> Vec<bool> {
    s.chars().filter(|c| *c != ' ').map(|c| c == '1').collect()
}

fn err_class(content: &[u8], mode: Mode, named: bool) -> ErrorClass {
    BitString::decode(content, mode, named).unwrap_err().class
}

// The value '0A3B5F291CD'H: 0000 1010 0011 1011 0101 1111 0010 1001 0001 1100 1101, 44 bits.
const ANNEX_BITS: &str = "0000 1010 0011 1011 0101 1111 0010 1001 0001 1100 1101";

// ---------------------------------------------------------------- primitive BIT STRING

#[test]
fn empty_value_is_a_single_zero_octet() {
    // V-S01: constructed by hand from X.690 §8.6.2.3: empty value, initial octet zero, no data octets.
    for mode in [BER, DER] {
        for named in [false, true] {
            let v = BitString::decode(&[0x00], mode, named).unwrap();
            assert_eq!(v.bit_len(), 0);
            assert!(v.is_empty());
            assert_eq!(bits_of(&v), vec![]);
            assert_eq!(v.bit(0), None);
        }
    }
}

#[test]
fn zero_unused_bits_whole_octet() {
    // V-S02: constructed by hand from X.690 §8.6.2.1 and §8.6.2.2 (0 unused bits).
    let v = BitString::decode(&[0x00, 0xFF], DER, false).unwrap();
    assert_eq!(v.bit_len(), 8);
    assert_eq!(bits_of(&v), bin("11111111"));
    assert_eq!(v.data(), &[0xFF]);
}

#[test]
fn seven_unused_bits_leaves_one_bit() {
    // V-S03: constructed by hand from X.690 §8.6.2.1 (leading bit in bit 8) and §8.6.2.2 (7 unused).
    let v = BitString::decode(&[0x07, 0x80], DER, false).unwrap();
    assert_eq!(v.bit_len(), 1);
    assert_eq!(v.unused_bits(), 7);
    assert_eq!(bits_of(&v), vec![true]);
    assert_eq!(v.bit(0), Some(true));
    assert_eq!(v.bit(1), None);
}

#[test]
fn a_single_zero_bit_is_valid_without_a_named_bit_list() {
    // V-S04: constructed by hand: bits(0). No NamedBitList, so no §11.2.2 trailing-zero removal.
    let v = BitString::decode(&[0x07, 0x00], DER, false).unwrap();
    assert_eq!(bits_of(&v), vec![false]);
}

#[test]
fn four_unused_zero_bits() {
    // V-S05: constructed by hand from X.690 §8.6.2.2 (4 unused bits, all zero).
    let v = BitString::decode(&[0x04, 0xF0], DER, false).unwrap();
    assert_eq!(bits_of(&v), bin("1111"));
}

#[test]
fn nonzero_pad_bits_are_accepted_in_ber_and_refused_in_der() {
    // V-S06: constructed by hand from X.690 §11.2.1 (unused bit 1 is set). BER is silent (primitives.md s4 item 4).
    let v = BitString::decode(&[0x04, 0xF1], BER, false).unwrap();
    assert_eq!(bits_of(&v), bin("1111"));
    assert_eq!(v.data(), &[0xF1], "the data is returned as received");
    // equal as values to the zero-padded form
    assert_eq!(v, BitString::decode(&[0x04, 0xF0], BER, false).unwrap());
    let e = BitString::decode(&[0x04, 0xF1], DER, false).unwrap_err();
    assert_eq!(e.class, ErrorClass::UnusedBitsNonzero);
    assert_eq!(e.offset, 1);
    // the highest unused bit alone, with 7 unused
    assert_eq!(
        err_class(&[0x07, 0x81], DER, false),
        ErrorClass::UnusedBitsNonzero
    );
    assert!(BitString::decode(&[0x07, 0x81], BER, false).is_ok());
    // pad bit set in a multi-octet value: offset is that of the final data octet
    let e = BitString::decode(&[0x01, 0xAA, 0xAB], DER, false).unwrap_err();
    assert_eq!((e.class, e.offset), (ErrorClass::UnusedBitsNonzero, 2));
}

#[test]
fn unequal_values_are_not_equal() {
    let a = BitString::decode(&[0x04, 0xF0], BER, false).unwrap();
    let b = BitString::decode(&[0x03, 0xF8], BER, false).unwrap();
    let c = BitString::decode(&[0x04, 0xE0], BER, false).unwrap();
    assert_ne!(a, b, "different bit lengths");
    assert_ne!(a, c, "different used bits");
}

#[test]
fn initial_octet_above_seven_is_bad_contents() {
    // V-S07 and V-S08: constructed by hand from X.690 §8.6.2.2 ¶2 (range 0..7); 8 and 255 are out of range.
    for content in [&[0x08u8, 0x00][..], &[0xFF, 0x00], &[0x09, 0xFF]] {
        for mode in [BER, DER] {
            let e = BitString::decode(content, mode, false).unwrap_err();
            assert_eq!((e.class, e.offset), (ErrorClass::BadContents, 0));
        }
    }
    // 8 with no data octet at all is also refused
    assert_eq!(err_class(&[0x08], BER, false), ErrorClass::BadContents);
}

#[test]
fn unused_bits_with_no_data_octets_is_bad_contents() {
    // V-S09: constructed by hand from X.690 §8.6.2.3 (empty value needs initial octet zero).
    for n in 1..=7u8 {
        assert_eq!(err_class(&[n], BER, false), ErrorClass::BadContents);
        assert_eq!(err_class(&[n], DER, true), ErrorClass::BadContents);
    }
}

#[test]
fn empty_contents_has_no_initial_octet() {
    // V-S10: constructed by hand from X.690 §8.6.2 ("shall contain an initial octet"): `03 00` has contents [].
    for mode in [BER, DER] {
        let e = BitString::decode(&[], mode, false).unwrap_err();
        assert_eq!((e.class, e.offset), (ErrorClass::BadContents, 0));
    }
}

#[test]
fn annex_value_primitive() {
    // V-S11: from X.690 §8.6.4.2 EXAMPLE: '0A3B5F291CD'H as `03 07 04 0A 3B 5F 29 1C D0`.
    let content = [0x04, 0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0];
    for mode in [BER, DER] {
        let v = BitString::decode(&content, mode, false).unwrap();
        assert_eq!(v.bit_len(), 44);
        assert_eq!(bits_of(&v), bin(ANNEX_BITS));
    }
}

#[test]
fn named_bit_list_in_der() {
    // V-S20: constructed by hand from X.690 §11.2.2 ¶1: bit 0 set, last bit is 1.
    assert!(BitString::decode(&[0x07, 0x80], DER, true).is_ok());
    // V-S21: constructed by hand from §11.2.2 ¶1: 9 bits 100000000, trailing zeros not removed.
    let e = BitString::decode(&[0x07, 0x80, 0x00], DER, true).unwrap_err();
    assert_eq!((e.class, e.offset), (ErrorClass::BitstringTrailingZeros, 2));
    // V-S22: constructed by hand from X.680 §22.7 (adding trailing zeros is permitted) and §11.2.2 scope:
    // BER accepts it as is, and DER accepts it when the type has no NamedBitList.
    let v = BitString::decode(&[0x07, 0x80, 0x00], BER, true).unwrap();
    assert_eq!(bits_of(&v), bin("100000000"));
    assert!(BitString::decode(&[0x07, 0x80, 0x00], DER, false).is_ok());
    // V-S23: constructed by hand from §11.2.2 ¶3 NOTE 2: eight zero bits must be `00`.
    assert_eq!(
        err_class(&[0x00, 0x00], DER, true),
        ErrorClass::BitstringTrailingZeros
    );
    assert_eq!(
        err_class(&[0x07, 0x00], DER, true),
        ErrorClass::BitstringTrailingZeros
    );
    assert!(BitString::decode(&[0x00, 0x00], DER, false).is_ok());
    assert!(BitString::decode(&[0x00, 0x00], BER, true).is_ok());
    // V-S24: constructed by hand from §11.2.2 ¶3 NOTE 2: the empty value is accepted.
    assert!(BitString::decode(&[0x00], DER, true).is_ok());
    // a last bit that is 1 in the middle of the final octet: 0x28 with 3 unused is `00101`, last bit 1
    assert!(BitString::decode(&[0x03, 0x28], DER, true).is_ok());
    // ... and 0x20 with 3 unused is `00100`, trailing zeros
    assert_eq!(
        err_class(&[0x03, 0x20], DER, true),
        ErrorClass::BitstringTrailingZeros
    );
    // a trailing-zero whole octet after a 1
    assert_eq!(
        err_class(&[0x00, 0x80, 0x00], DER, true),
        ErrorClass::BitstringTrailingZeros
    );
}

#[test]
fn der_pad_check_comes_before_the_trailing_zero_check() {
    // `07 01`: bit 0 is 0 and a pad bit is set; the pad rule is the more basic one (§11.2.1 applies always).
    assert_eq!(
        err_class(&[0x07, 0x01], DER, true),
        ErrorClass::UnusedBitsNonzero
    );
}

#[test]
fn encode_writes_literal_contents() {
    let mut out = [0xEEu8; 16];
    // empty value: `00` (§8.6.2.3)
    assert_eq!(bitstring::encode(&[], 0, &mut out), Ok(1));
    assert_eq!(&out[..1], &[0x00]);
    // 1 bit, junk past the bit in the source: pad is zeroed (§11.2.1)
    assert_eq!(bitstring::encode(&[0xFF], 1, &mut out), Ok(2));
    assert_eq!(&out[..2], &[0x07, 0x80]);
    // 8 bits: 0 unused
    assert_eq!(bitstring::encode(&[0xFF], 8, &mut out), Ok(2));
    assert_eq!(&out[..2], &[0x00, 0xFF]);
    // 4 bits
    assert_eq!(bitstring::encode(&[0xFF], 4, &mut out), Ok(2));
    assert_eq!(&out[..2], &[0x04, 0xF0]);
    // 9 bits: two data octets, 7 unused
    assert_eq!(bitstring::encode(&[0xFF, 0xFF], 9, &mut out), Ok(3));
    assert_eq!(&out[..3], &[0x07, 0xFF, 0x80]);
    // the annex value, 44 bits, from X.690 §8.6.4.2: `04 0A 3B 5F 29 1C D0`
    let src = [0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xDF];
    assert_eq!(bitstring::encode(&src, 44, &mut out), Ok(7));
    assert_eq!(&out[..7], &[0x04, 0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0]);
    // extra source octets beyond the bit length are ignored
    assert_eq!(bitstring::encode(&[0x80, 0xFF, 0xFF], 1, &mut out), Ok(2));
    assert_eq!(&out[..2], &[0x07, 0x80]);
}

#[test]
fn encode_refuses_short_input_and_short_output() {
    let mut out = [0u8; 2];
    assert_eq!(
        bitstring::encode(&[0xFF], 9, &mut out).unwrap_err().class,
        ErrorClass::BadContents
    );
    assert_eq!(
        bitstring::encode(&[0xFF, 0xFF], 9, &mut out)
            .unwrap_err()
            .class,
        ErrorClass::LimitExceeded
    );
    let mut none = [0u8; 0];
    assert_eq!(
        bitstring::encode(&[], 0, &mut none).unwrap_err().class,
        ErrorClass::LimitExceeded
    );
}

#[test]
fn encode_then_decode_round_trips_in_der() {
    let src = [0b1010_0000u8];
    let mut out = [0u8; 4];
    let n = bitstring::encode(&src, 3, &mut out).unwrap();
    assert_eq!(&out[..n], &[0x05, 0xA0]);
    let v = BitString::decode(&out[..n], DER, false).unwrap();
    assert_eq!(bits_of(&v), bin("101"));
}

#[test]
fn named_bit_list_len_strips_trailing_zero_bits() {
    // X.690 §11.2.2: all trailing 0 bits removed; NOTE 2: no 1 bits gives the empty value.
    assert_eq!(bitstring::named_bit_list_len(&[0b1010_0000], 8), 3);
    assert_eq!(bitstring::named_bit_list_len(&[0x80, 0x00], 16), 1);
    assert_eq!(bitstring::named_bit_list_len(&[0x00, 0x00], 16), 0);
    assert_eq!(bitstring::named_bit_list_len(&[0x01], 8), 8);
    assert_eq!(bitstring::named_bit_list_len(&[0xFF], 4), 4);
    assert_eq!(
        bitstring::named_bit_list_len(&[0x0F], 4),
        0,
        "bits past the length are ignored"
    );
    assert_eq!(bitstring::named_bit_list_len(&[], 0), 0);
    assert_eq!(
        bitstring::named_bit_list_len(&[0x80], 100),
        1,
        "length clamped to the slice"
    );
}

#[test]
fn named_bit_list_value_with_and_without_trailing_zeros() {
    // constructed by hand: a flag type with bits 0 and 2 set, as 5 bits `10100` (trailing zeros) or 3 bits `101`.
    let src = [0b1010_0000u8];
    let mut with_zeros = [0u8; 4];
    let n = bitstring::encode(&src, 5, &mut with_zeros).unwrap();
    assert_eq!(&with_zeros[..n], &[0x03, 0xA0]);
    assert_eq!(
        err_class(&with_zeros[..n], DER, true),
        ErrorClass::BitstringTrailingZeros
    );
    assert!(BitString::decode(&with_zeros[..n], BER, true).is_ok());
    let mut stripped = [0u8; 4];
    let len = bitstring::named_bit_list_len(&src, 5);
    assert_eq!(len, 3);
    let n = bitstring::encode(&src, len, &mut stripped).unwrap();
    assert_eq!(&stripped[..n], &[0x05, 0xA0]);
    assert!(BitString::decode(&stripped[..n], DER, true).is_ok());
}

// ---------------------------------------------------------------- constructed BIT STRING

fn constructed<'a>(
    segments: &'a [&'a [u8]],
    mode: Mode,
) -> Result<ConstructedBitString<impl Iterator<Item = &'a [u8]> + Clone>, berkit::Error> {
    ConstructedBitString::new(segments.iter().copied(), mode)
}

#[test]
fn annex_value_constructed() {
    // V-S12 / V-S13: from X.690 §8.6.4.2 EXAMPLE (second form), segments `03 03 00 0A 3B` and
    // `03 05 04 5F 29 1C D0`, i.e. contents `00 0A 3B` and `04 5F 29 1C D0`.
    let segs: [&[u8]; 2] = [&[0x00, 0x0A, 0x3B], &[0x04, 0x5F, 0x29, 0x1C, 0xD0]];
    let c = constructed(&segs, BER).unwrap();
    assert_eq!(c.bit_len(), 44);
    assert_eq!(c.octet_len(), 6);
    assert_eq!(c.unused_bits(), 4);
    assert_eq!(c.bits().collect::<Vec<_>>(), bin(ANNEX_BITS));
    assert_eq!(
        c.data_octets().collect::<Vec<_>>(),
        vec![0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0]
    );
    // reassembled data equals the primitive form's data
    let mut out = [0u8; 8];
    let n = c.copy_data(&mut out).unwrap();
    assert_eq!(&out[..n], &[0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0]);
    let prim = BitString::decode(&[0x04, 0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0], BER, false).unwrap();
    assert_eq!(c.bits().collect::<Vec<_>>(), bits_of(&prim));
    // DER refuses the constructed form
    let e = constructed(&segs, DER).err().unwrap();
    assert_eq!(e.class, ErrorClass::ConstructedString);
}

#[test]
fn constructed_zero_segments_is_the_empty_value() {
    // V-S14 / V-S15: constructed by hand from X.690 §8.6.3 (zero nested encodings).
    let c = constructed(&[], BER).unwrap();
    assert_eq!((c.bit_len(), c.octet_len(), c.unused_bits()), (0, 0, 0));
    assert_eq!(c.bits().count(), 0);
    assert_eq!(
        constructed(&[], DER).err().unwrap().class,
        ErrorClass::ConstructedString
    );
}

#[test]
fn constructed_empty_segments_are_allowed() {
    // V-S16: constructed by hand from X.690 §8.6.4 NOTE: segments `00` and `00 FF`.
    let segs: [&[u8]; 2] = [&[0x00], &[0x00, 0xFF]];
    let c = constructed(&segs, BER).unwrap();
    assert_eq!(c.bit_len(), 8);
    assert_eq!(c.bits().collect::<Vec<_>>(), bin("11111111"));
    // an empty segment last, after data
    let segs: [&[u8]; 2] = [&[0x00, 0xFF], &[0x00]];
    let c = constructed(&segs, BER).unwrap();
    assert_eq!((c.bit_len(), c.unused_bits()), (8, 0));
    // an empty segment between two others
    let segs: [&[u8]; 3] = [&[0x00, 0xAA], &[0x00], &[0x03, 0xF8]];
    let c = constructed(&segs, BER).unwrap();
    assert_eq!(c.bit_len(), 13);
    assert_eq!(c.bits().collect::<Vec<_>>(), bin("10101010 11111"));
    // only empty segments
    let segs: [&[u8]; 2] = [&[0x00], &[0x00]];
    assert_eq!(constructed(&segs, BER).unwrap().bit_len(), 0);
}

#[test]
fn constructed_non_final_segment_with_unused_bits_is_bad_contents() {
    // V-S17: constructed by hand from X.690 §8.6.4 ¶2: `04 F0` is not last and is not a multiple of 8 bits.
    let segs: [&[u8]; 2] = [&[0x04, 0xF0], &[0x00, 0xFF]];
    let e = constructed(&segs, BER).err().unwrap();
    assert_eq!((e.class, e.offset), (ErrorClass::BadContents, 0));
    // an unused-bit count of 1 in the middle of three
    let segs: [&[u8]; 3] = [&[0x00, 0xAA], &[0x01, 0xFE], &[0x00, 0xFF]];
    let e = constructed(&segs, BER).err().unwrap();
    assert_eq!((e.class, e.offset), (ErrorClass::BadContents, 1));
    // the same segment as the LAST one is fine
    let segs: [&[u8]; 2] = [&[0x00, 0xAA], &[0x01, 0xFE]];
    assert_eq!(constructed(&segs, BER).unwrap().bit_len(), 15);
    // a non-final segment with unused bits is refused even when the final one is empty
    let segs: [&[u8]; 2] = [&[0x04, 0xF0], &[0x00]];
    assert_eq!(
        constructed(&segs, BER).err().unwrap().class,
        ErrorClass::BadContents
    );
}

#[test]
fn constructed_nested_segments_flattened_by_the_caller() {
    // V-S18: constructed by hand from X.690 §8.6.4.1 ¶1-¶4: the outer segment `23 80 03 02 00 FF 00 00` is itself
    // constructed; the caller yields its leaf `00 FF`, then the leaf `00 AA`.
    let segs: [&[u8]; 2] = [&[0x00, 0xFF], &[0x00, 0xAA]];
    let c = constructed(&segs, BER).unwrap();
    assert_eq!(c.bit_len(), 16);
    assert_eq!(c.bits().collect::<Vec<_>>(), bin("11111111 10101010"));
    // a nested constructed segment ending in unused bits that is followed by more data: flattened it is a
    // non-final leaf with unused bits, which is the recursive rule's violation too (the nested segment is
    // not a multiple of 8 bits).
    let segs: [&[u8]; 3] = [&[0x00, 0xFF], &[0x03, 0xF8], &[0x00, 0xAA]];
    assert_eq!(
        constructed(&segs, BER).err().unwrap().class,
        ErrorClass::BadContents
    );
    // nested, with an empty leaf closing the nested segment: whole octets overall, fine
    let segs: [&[u8]; 3] = [&[0x00, 0xFF], &[0x00], &[0x04, 0xA0]];
    let c = constructed(&segs, BER).unwrap();
    assert_eq!((c.bit_len(), c.unused_bits()), (12, 4));
}

#[test]
fn constructed_segment_contents_are_validated() {
    // empty segment contents (no initial octet), initial octet above 7, unused bits with no data: each at its index
    let segs: [&[u8]; 2] = [&[0x00, 0xFF], &[]];
    let e = constructed(&segs, BER).err().unwrap();
    assert_eq!((e.class, e.offset), (ErrorClass::BadContents, 1));
    let segs: [&[u8]; 1] = [&[0x08, 0xFF]];
    assert_eq!(
        constructed(&segs, BER).err().unwrap().class,
        ErrorClass::BadContents
    );
    let segs: [&[u8]; 1] = [&[0x03]];
    assert_eq!(
        constructed(&segs, BER).err().unwrap().class,
        ErrorClass::BadContents
    );
}

#[test]
fn constructed_final_pad_bits_are_masked_on_reassembly() {
    // BER does not constrain pad values; the reassembled data is zero-padded, the iterator is as supplied.
    let segs: [&[u8]; 2] = [&[0x00, 0xAA], &[0x04, 0xFF]];
    let c = constructed(&segs, BER).unwrap();
    assert_eq!(c.data_octets().collect::<Vec<_>>(), vec![0xAA, 0xFF]);
    let mut out = [0u8; 2];
    assert_eq!(c.copy_data(&mut out), Ok(2));
    assert_eq!(out, [0xAA, 0xF0]);
    assert_eq!(c.bits().count(), 12);
    let mut small = [0u8; 1];
    assert_eq!(
        c.copy_data(&mut small).unwrap_err().class,
        ErrorClass::LimitExceeded
    );
}

#[test]
fn constructed_acceptable_by_mode() {
    // X.690 §10.2: constructed form is not used in DER; §8.6.1: sender's option in BER.
    assert!(bitstring::constructed_allowed(BER));
    assert!(!bitstring::constructed_allowed(DER));
    assert_eq!(bitstring::check_constructed(BER, 5), Ok(()));
    let e = bitstring::check_constructed(DER, 5).unwrap_err();
    assert_eq!((e.class, e.offset), (ErrorClass::ConstructedString, 5));
    assert!(octetstring::constructed_allowed(BER));
    assert!(!octetstring::constructed_allowed(DER));
    assert_eq!(octetstring::check_constructed(BER, 0), Ok(()));
    assert_eq!(
        octetstring::check_constructed(DER, 0).unwrap_err().class,
        ErrorClass::ConstructedString
    );
}

// ---------------------------------------------------------------- OCTET STRING

#[test]
fn primitive_octet_string() {
    // V-O01: constructed by hand from X.690 §8.7.2: empty. V-O02: three octets.
    assert_eq!(octetstring::decode(&[]), &[] as &[u8]);
    assert_eq!(octetstring::decode(&[1, 2, 3]), &[1, 2, 3]);
    let mut out = [0xEEu8; 4];
    assert_eq!(octetstring::encode(&[], &mut out), Ok(0));
    assert_eq!(octetstring::encode(&[1, 2, 3], &mut out), Ok(3));
    assert_eq!(out, [1, 2, 3, 0xEE]);
    let mut small = [0u8; 2];
    assert_eq!(
        octetstring::encode(&[1, 2, 3], &mut small)
            .unwrap_err()
            .class,
        ErrorClass::LimitExceeded
    );
}

fn oct<'a>(
    segments: &'a [&'a [u8]],
    mode: Mode,
) -> Result<ConstructedOctetString<impl Iterator<Item = &'a [u8]> + Clone>, berkit::Error> {
    ConstructedOctetString::new(segments.iter().copied(), mode)
}

#[test]
fn constructed_octet_string_concatenates() {
    // V-O03 / V-O04: constructed by hand from X.690 §8.7.3.1-§8.7.3.2: segments `01 02` and `03`.
    let segs: [&[u8]; 2] = [&[1, 2], &[3]];
    let c = oct(&segs, BER).unwrap();
    assert_eq!(c.len(), 3);
    assert!(!c.is_empty());
    assert_eq!(c.octets().collect::<Vec<_>>(), vec![1, 2, 3]);
    let mut out = [0u8; 3];
    assert_eq!(c.copy_into(&mut out), Ok(3));
    assert_eq!(out, [1, 2, 3]);
    let mut small = [0u8; 2];
    assert_eq!(
        c.copy_into(&mut small).unwrap_err().class,
        ErrorClass::LimitExceeded
    );
    // DER refuses (§10.2)
    assert_eq!(
        oct(&segs, DER).err().unwrap().class,
        ErrorClass::ConstructedString
    );
}

#[test]
fn constructed_octet_string_nested_empty_and_none() {
    // V-O05: constructed by hand from §8.7.3.2 ¶1-¶4: nested `24 80 04 01 AA 00 00` then `04 01 BB`, flattened.
    let segs: [&[u8]; 2] = [&[0xAA], &[0xBB]];
    assert_eq!(
        oct(&segs, BER).unwrap().octets().collect::<Vec<_>>(),
        vec![0xAA, 0xBB]
    );
    // V-O07: constructed by hand from §8.7.3 ¶1: zero encodings.
    let c = oct(&[], BER).unwrap();
    assert!(c.is_empty());
    assert_eq!(c.octets().count(), 0);
    assert_eq!(
        oct(&[], DER).err().unwrap().class,
        ErrorClass::ConstructedString
    );
    // V-O08: constructed by hand from §8.7.3.1 NOTE: an empty segment between others.
    let segs: [&[u8]; 3] = [&[], &[0x07], &[]];
    let c = oct(&segs, BER).unwrap();
    assert_eq!(c.octets().collect::<Vec<_>>(), vec![0x07]);
    assert_eq!(c.len(), 1);
}

// V-S19 and V-O06 (wrong inner tag) belong to the TLV layer, which sees the identifiers; this module
// receives contents only. See roadmap/bitoctet.md.
