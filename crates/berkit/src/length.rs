//! Length octets: short, long and indefinite forms. X.690 §8.1.3, §10.1.
//!
//! A decoded definite length is a `u64`. BER accepts every alternative the sender may choose (X.690 §7.3 ¶2),
//! which includes non-minimal long forms with leading zero octets and more than eight length octets
//! (§8.1.3.5 NOTE 2), so the limit applies to the VALUE, never to the number of octets. DER takes the minimal
//! form only (§10.1).

use crate::tag::BufferTooSmall;
use crate::{Error, ErrorClass, Mode};
use core::fmt;

/// A decoded length. X.690 §8.1.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Length {
    /// The definite form: the number of contents octets. X.690 §8.1.3.3.
    Definite(u64),
    /// The indefinite form, the single octet `80`. X.690 §8.1.3.6. Contents end at the end-of-contents octets.
    Indefinite,
}

/// The result of [`decode_raw`]: the length, the octets consumed and whether the form is the minimal one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// The length value or the indefinite marker.
    pub length: Length,
    /// The length octets consumed, including the initial octet.
    pub consumed: usize,
    /// True when the definite form is the one DER requires (X.690 §10.1): the short form for 0 to 127, else
    /// the long form with the fewest octets and no leading zero octet. Always true for the indefinite form.
    pub minimal: bool,
}

/// The number of octets that hold `value` with no leading zero octet (at least 1).
const fn octets_for(value: u64) -> usize {
    let bits = 64 - value.leading_zeros() as usize;
    if bits == 0 { 1 } else { bits.div_ceil(8) }
}

/// Decode the length octets at the start of `input` and report whether the form is minimal, without applying
/// any DER rule. X.690 §8.1.3.
///
/// Errors (offsets from the start of `input`): `truncated` when the announced length octets are not all
/// present (§8.1.3.5); `reserved_length` for the initial octet `FF` (§8.1.3.5 ¶2 c); `limit_exceeded` when the
/// value does not fit a `u64` after the leading zero octets are ignored (POLICY: the standard sets no bound,
/// and §8.1.3.5 allows up to 126 length octets).
///
/// # Errors
/// See above.
pub fn decode_raw(input: &[u8]) -> Result<Decoded, Error> {
    let Some(&first) = input.first() else {
        return Err(Error::new(ErrorClass::Truncated, 0));
    };
    if first < 0x80 {
        // Short form, X.690 §8.1.3.4.
        return Ok(Decoded {
            length: Length::Definite(u64::from(first)),
            consumed: 1,
            minimal: true,
        });
    }
    if first == 0x80 {
        // Indefinite form, X.690 §8.1.3.6.1. `81 00` is a long form of zero, handled below.
        return Ok(Decoded {
            length: Length::Indefinite,
            consumed: 1,
            minimal: true,
        });
    }
    if first == 0xFF {
        return Err(Error::new(ErrorClass::ReservedLength, 0));
    }
    let count = usize::from(first & 0x7F);
    let Some(octets) = input.get(1..=count) else {
        return Err(Error::new(ErrorClass::Truncated, input.len()));
    };
    let mut value: u64 = 0;
    for &octet in octets {
        if value >> 56 != 0 {
            return Err(Error::new(ErrorClass::LimitExceeded, 0));
        }
        value = (value << 8) | u64::from(octet);
    }
    let minimal = value >= 128 && count == octets_for(value);
    Ok(Decoded {
        length: Length::Definite(value),
        consumed: 1 + count,
        minimal,
    })
}

/// Decode the length octets at the start of `input` under `mode`. Returns the length and the octets consumed.
///
/// BER accepts the short form, any long form (including non-minimal ones) and the indefinite form. DER
/// (X.690 §10.1) refuses the indefinite form with `indefinite_length` and a definite length that is not in
/// the fewest octets with `non_minimal_length`. The errors of [`decode_raw`] apply in both modes.
///
/// # Errors
/// See above.
pub fn decode(input: &[u8], mode: Mode) -> Result<(Length, usize), Error> {
    let d = decode_raw(input)?;
    if mode.is_der() {
        if d.length == Length::Indefinite {
            return Err(Error::new(ErrorClass::IndefiniteLength, 0));
        }
        if !d.minimal {
            return Err(Error::new(ErrorClass::NonMinimalLength, 0));
        }
    }
    Ok((d.length, d.consumed))
}

impl Length {
    /// The number of length octets of the minimal encoding. X.690 §8.1.3.4, §8.1.3.5, §10.1.
    #[must_use]
    pub const fn encoded_len(&self) -> usize {
        match *self {
            Self::Indefinite => 1,
            Self::Definite(v) if v < 128 => 1,
            Self::Definite(v) => 1 + octets_for(v),
        }
    }

    /// Write the minimal length octets into `out` and return the octets written: the short form for 0 to 127,
    /// else the long form with the fewest octets, and `80` for the indefinite form. X.690 §8.1.3.4, §8.1.3.5,
    /// §8.1.3.6.1, §10.1.
    ///
    /// # Errors
    /// [`BufferTooSmall`] when `out` is shorter than [`Length::encoded_len`].
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, BufferTooSmall> {
        let len = self.encoded_len();
        if out.len() < len {
            return Err(BufferTooSmall { needed: len });
        }
        match *self {
            Self::Indefinite => out[0] = 0x80,
            Self::Definite(v) if v < 128 => out[0] = v.to_be_bytes()[7],
            Self::Definite(v) => {
                let count = len - 1;
                // `count` is 1 to 8, so the narrowing is exact.
                #[allow(clippy::cast_possible_truncation)]
                let initial = 0x80 | count as u8;
                out[0] = initial;
                out[1..len].copy_from_slice(&v.to_be_bytes()[8 - count..]);
            }
        }
        Ok(len)
    }
}

/// Why a long-form length could not be written by [`encode_long`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongFormError {
    /// The output buffer is too small.
    BufferTooSmall(BufferTooSmall),
    /// The number of length octets is 0, or above 126 (the initial octet `FF` is reserved). X.690 §8.1.3.5 c).
    OctetCount,
    /// The value does not fit in the requested number of length octets.
    ValueTooLarge,
}

impl From<BufferTooSmall> for LongFormError {
    fn from(e: BufferTooSmall) -> Self {
        Self::BufferTooSmall(e)
    }
}

impl fmt::Display for LongFormError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall(e) => e.fmt(f),
            Self::OctetCount => f.write_str("a long-form length has 1 to 126 length octets"),
            Self::ValueTooLarge => f.write_str("the length does not fit in that many octets"),
        }
    }
}

impl core::error::Error for LongFormError {}

/// Write the definite length `value` in the long form with exactly `octets` subsequent length octets, padded on
/// the left with zero octets, and return the octets written (`1 + octets`).
///
/// A BER sender may choose this form (X.690 §8.1.3.5 NOTE 2: the length octets may have leading zero octets and
/// there may be more than eight of them); DER forbids it and takes the minimal form only (§10.1), which is what
/// [`Length::encode`] writes. A long-form zero (`81 00`) is a definite length of 0, not the indefinite form.
///
/// # Errors
/// [`LongFormError::OctetCount`] unless `octets` is 1 to 126, [`LongFormError::ValueTooLarge`] when `value` needs
/// more than `octets` octets, [`LongFormError::BufferTooSmall`] when `out` is shorter than `1 + octets`.
pub fn encode_long(value: u64, octets: usize, out: &mut [u8]) -> Result<usize, LongFormError> {
    if !(1..=126).contains(&octets) {
        return Err(LongFormError::OctetCount);
    }
    if octets < octets_for(value) {
        return Err(LongFormError::ValueTooLarge);
    }
    let total = 1 + octets;
    if out.len() < total {
        return Err(BufferTooSmall { needed: total }.into());
    }
    // `octets` is 1 to 126, so the narrowing is exact.
    #[allow(clippy::cast_possible_truncation)]
    let initial = 0x80 | octets as u8;
    out[0] = initial;
    out[1..total].fill(0);
    let be = value.to_be_bytes();
    let used = octets.min(8);
    out[total - used..total].copy_from_slice(&be[8 - used..]);
    Ok(total)
}
