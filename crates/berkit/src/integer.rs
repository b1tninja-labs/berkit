//! INTEGER and ENUMERATED contents: two's-complement minimality. X.690 §8.3, §8.4.
//!
//! ENUMERATED is encoded as the associated integer (§8.4), so both types share this module. The contents are a
//! big-endian two's-complement number in the fewest octets (§8.3.2). The standard sets no size limit, so
//! [`Integer`] borrows the contents of any length and the fixed-width conversions report overflow as `None`.

use crate::{Error, ErrorClass, Mode};

/// A decoded INTEGER or ENUMERATED: a borrowed view of minimal two's-complement contents. X.690 §8.3.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Integer<'a> {
    bytes: &'a [u8],
}

/// True when the octet pair `a`, `b` makes `a` redundant: the first octet and bit 8 of the second are all
/// zero or all ones. X.690 §8.3.2.
const fn redundant(a: u8, b: u8) -> bool {
    (a == 0x00 && b & 0x80 == 0) || (a == 0xFF && b & 0x80 != 0)
}

/// Decode the contents octets of an INTEGER or ENUMERATED.
///
/// The contents are never empty (X.690 §8.3.1) and, with more than one octet, the first octet and bit 8 of the
/// second are neither all ones nor all zero (§8.3.2). POLICY: both rules are "shall" with no sender's option, so
/// they bind BER as well as DER and `mode` does not change the result (see ROADMAP, policy 1).
///
/// # Errors
/// `bad_contents` for empty contents, `non_minimal_integer` (offset 0) for a redundant leading octet.
pub fn decode(content: &[u8], _mode: Mode) -> Result<Integer<'_>, Error> {
    match *content {
        [] => Err(Error::new(ErrorClass::BadContents, 0)),
        [a, b, ..] if redundant(a, b) => Err(Error::new(ErrorClass::NonMinimalInteger, 0)),
        _ => Ok(Integer { bytes: content }),
    }
}

impl<'a> Integer<'a> {
    /// The contents octets, unchanged: big-endian two's complement. X.690 §8.3.3.
    #[must_use]
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// True when the value is below zero: bit 8 of the first octet is set. X.690 §8.3.3.
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        self.bytes[0] & 0x80 != 0
    }

    /// True when the value is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.bytes == [0x00]
    }

    /// The value as `i128`, or `None` if it does not fit.
    #[must_use]
    pub fn to_i128(&self) -> Option<i128> {
        if self.bytes.len() > 16 {
            return None;
        }
        let fill = if self.is_negative() { 0xFF } else { 0x00 };
        let mut buf = [fill; 16];
        buf[16 - self.bytes.len()..].copy_from_slice(self.bytes);
        Some(i128::from_be_bytes(buf))
    }

    /// The value as `u128`, or `None` if it is negative or does not fit.
    #[must_use]
    pub fn to_u128(&self) -> Option<u128> {
        let magnitude = self.unsigned_bytes()?;
        if magnitude.len() > 16 {
            return None;
        }
        let mut buf = [0u8; 16];
        buf[16 - magnitude.len()..].copy_from_slice(magnitude);
        Some(u128::from_be_bytes(buf))
    }

    /// The value as `i64`, or `None` if it does not fit.
    #[must_use]
    pub fn to_i64(&self) -> Option<i64> {
        if self.bytes.len() > 8 {
            return None;
        }
        i64::try_from(self.to_i128()?).ok()
    }

    /// The value as `u64`, or `None` if it is negative or does not fit.
    #[must_use]
    pub fn to_u64(&self) -> Option<u64> {
        u64::try_from(self.to_u128()?).ok()
    }

    /// For a value of zero or more, its big-endian magnitude without the sign octet (zero is `[0]`); `None` if
    /// negative. Borrowed from the contents.
    #[must_use]
    pub fn unsigned_bytes(&self) -> Option<&'a [u8]> {
        if self.is_negative() {
            None
        } else if self.bytes.len() > 1 && self.bytes[0] == 0x00 {
            Some(&self.bytes[1..])
        } else {
            Some(self.bytes)
        }
    }

    /// Write the absolute value, big-endian with no leading zero octet (zero is a single `00`), into `out`.
    /// Returns the octets written, or `None` if `out` is too small (`as_bytes().len()` octets always suffice).
    #[must_use]
    pub fn write_magnitude(&self, out: &mut [u8]) -> Option<usize> {
        let n = self.bytes.len();
        let dst = out.get_mut(..n)?;
        dst.copy_from_slice(self.bytes);
        if self.is_negative() {
            negate(dst);
        }
        let lead = dst.iter().take(n - 1).take_while(|&&b| b == 0).count();
        dst.copy_within(lead.., 0);
        Some(n - lead)
    }
}

/// Two's-complement negation in place (invert, add one).
fn negate(bytes: &mut [u8]) {
    let mut carry = true;
    for b in bytes.iter_mut().rev() {
        *b = !*b;
        if carry {
            let (v, c) = b.overflowing_add(1);
            *b = v;
            carry = c;
        }
    }
}

/// Write the minimal contents of an `i128`. X.690 §8.3.2. Returns the octets written, or `None` if `out` is
/// too small.
#[must_use]
pub fn encode_i128(value: i128, out: &mut [u8]) -> Option<usize> {
    let be = value.to_be_bytes();
    let mut start = 0;
    while start < 15 && redundant(be[start], be[start + 1]) {
        start += 1;
    }
    let src = &be[start..];
    out.get_mut(..src.len())?.copy_from_slice(src);
    Some(src.len())
}

/// Write the minimal contents of an `i64`. X.690 §8.3.2. Returns the octets written, or `None` if `out` is
/// too small.
#[must_use]
pub fn encode_i64(value: i64, out: &mut [u8]) -> Option<usize> {
    encode_i128(i128::from(value), out)
}

/// Write the minimal contents of a `u64`; a leading `00` is added when bit 8 of the first octet would
/// otherwise read as a sign. X.690 §8.3.2, §8.3.3. Returns the octets written, or `None` if `out` is too small.
#[must_use]
pub fn encode_u64(value: u64, out: &mut [u8]) -> Option<usize> {
    encode_i128(i128::from(value), out)
}

/// Write the minimal contents of the number whose absolute value is the big-endian `magnitude` and whose sign
/// is `negative`. X.690 §8.3.2, §8.3.3.
///
/// Leading zero octets of `magnitude` are ignored, and a zero magnitude is the single octet `00` whatever the
/// sign (there is no negative zero). Returns the octets written (at most `magnitude.len() + 1`), or `None` if
/// `out` is too small.
#[must_use]
pub fn encode_magnitude(negative: bool, magnitude: &[u8], out: &mut [u8]) -> Option<usize> {
    let lead = magnitude.iter().take_while(|&&b| b == 0).count();
    let m = &magnitude[lead..];
    if m.is_empty() {
        *out.first_mut()? = 0x00;
        return Some(1);
    }
    let n = m.len();
    let dst = out.get_mut(..=n)?;
    dst[0] = if negative { 0xFF } else { 0x00 };
    dst[1..].copy_from_slice(m);
    if negative {
        negate(&mut dst[1..]);
    }
    // The prefix octet is redundant when the next octet already carries the sign.
    if redundant(dst[0], dst[1]) {
        dst.copy_within(1.., 0);
        Some(n)
    } else {
        Some(n + 1)
    }
}
