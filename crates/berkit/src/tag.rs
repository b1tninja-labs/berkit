//! Identifier octets: class, form and tag number, including the high-tag-number form. X.690 §8.1.2.
//!
//! The tag number is a `u64`: X.690 §8.1.2.4.2 b) sets no maximum, so the width is a POLICY. 32 bits would
//! refuse tag numbers the standard allows, and an unbounded number cannot be held without allocation. A
//! number above `u64::MAX` is `limit_exceeded`.

use crate::{Error, ErrorClass};
use core::fmt;

/// The output buffer given to an encoder is smaller than the encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferTooSmall {
    /// The number of octets the encoding needs.
    pub needed: usize,
}

impl fmt::Display for BufferTooSmall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "output buffer too small: {} octets needed", self.needed)
    }
}

impl core::error::Error for BufferTooSmall {}

/// A tag class. X.690 §8.1.2.2 Table 1 (bits 8 and 7 of the leading identifier octet).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    /// Bits 8-7 are `00`.
    Universal,
    /// Bits 8-7 are `01`.
    Application,
    /// Bits 8-7 are `10`.
    ContextSpecific,
    /// Bits 8-7 are `11`.
    Private,
}

impl Class {
    /// The class for the two class bits (the value of bits 8 and 7, 0 to 3). X.690 §8.1.2.2 Table 1.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::Universal,
            1 => Self::Application,
            2 => Self::ContextSpecific,
            _ => Self::Private,
        }
    }

    /// The two class bits, 0 to 3. X.690 §8.1.2.2 Table 1.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::Universal => 0,
            Self::Application => 1,
            Self::ContextSpecific => 2,
            Self::Private => 3,
        }
    }
}

/// The decoded identifier octets of one encoding. X.690 §8.1.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tag {
    /// Bits 8-7 of the leading octet. X.690 §8.1.2.2.
    pub class: Class,
    /// Bit 6 of the leading octet: 1 is constructed. X.690 §8.1.2.5.
    pub constructed: bool,
    /// The tag number. A number 0 to 30 is the single-octet form, 31 and above the high-tag-number form.
    /// X.690 §8.1.2.2, §8.1.2.4.
    pub number: u64,
}

impl Tag {
    /// A tag from its three parts. X.690 §8.1.2.1.
    #[must_use]
    pub const fn new(class: Class, constructed: bool, number: u64) -> Self {
        Self {
            class,
            constructed,
            number,
        }
    }

    /// True for UNIVERSAL 0, which is reserved for the end-of-contents octets (X.680 §8.4 Table 1,
    /// X.690 §8.1.5). It is legal only as the EOC closing an indefinite-length encoding.
    #[must_use]
    pub const fn is_universal_zero(&self) -> bool {
        matches!(self.class, Class::Universal) && self.number == 0
    }

    /// Decode the identifier octets at the start of `input`. Returns the tag and the octets consumed.
    ///
    /// X.690 §8.1.2. Errors, with offsets counted from the start of `input`:
    /// `truncated` (input ends inside the identifier, §8.1.2.4.2 a);
    /// `non_minimal_tag` (a first subsequent octet with bits 7-1 all zero, §8.1.2.4.2 c), or a number 0 to 30
    /// in the high form, §8.1.2.2 with §8.1.2.4 (derived: no one sentence says "reject");
    /// `limit_exceeded` (a number above `u64::MAX`; POLICY, the standard sets no maximum).
    ///
    /// # Errors
    /// See above.
    pub fn decode(input: &[u8]) -> Result<(Self, usize), Error> {
        let Some(&lead) = input.first() else {
            return Err(Error::new(ErrorClass::Truncated, 0));
        };
        let class = Class::from_bits(lead >> 6);
        let constructed = lead & 0x20 != 0;
        let low = lead & 0x1F;
        if low != 0x1F {
            return Ok((Self::new(class, constructed, u64::from(low)), 1));
        }
        let mut number: u64 = 0;
        let mut i = 1;
        loop {
            let Some(&octet) = input.get(i) else {
                return Err(Error::new(ErrorClass::Truncated, input.len()));
            };
            let group = octet & 0x7F;
            if i == 1 && group == 0 {
                return Err(Error::new(ErrorClass::NonMinimalTag, 1));
            }
            if number >> 57 != 0 {
                return Err(Error::new(ErrorClass::LimitExceeded, i));
            }
            number = (number << 7) | u64::from(group);
            i += 1;
            if octet & 0x80 == 0 {
                break;
            }
        }
        if number < 31 {
            return Err(Error::new(ErrorClass::NonMinimalTag, 1));
        }
        Ok((Self::new(class, constructed, number), i))
    }

    /// The number of identifier octets this tag takes: 1 for a number 0 to 30, else 2 to 11. X.690 §8.1.2.
    #[must_use]
    pub const fn encoded_len(&self) -> usize {
        if self.number < 31 {
            1
        } else {
            let bits = 64 - self.number.leading_zeros() as usize;
            1 + bits.div_ceil(7)
        }
    }

    /// Write the identifier octets into `out`, always in the minimal form, and return the octets written.
    /// X.690 §8.1.2.2 (numbers 0 to 30), §8.1.2.4.2 (the base-128 groups, bit 8 set on all but the last).
    ///
    /// # Errors
    /// [`BufferTooSmall`] when `out` is shorter than [`Tag::encoded_len`].
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, BufferTooSmall> {
        let len = self.encoded_len();
        if out.len() < len {
            return Err(BufferTooSmall { needed: len });
        }
        let lead = (self.class.bits() << 6) | (u8::from(self.constructed) << 5);
        if self.number < 31 {
            // The number is below 31, so it fits bits 5-1.
            out[0] = lead | (self.number & 0x1F) as u8;
            return Ok(1);
        }
        out[0] = lead | 0x1F;
        for (k, slot) in out[1..len].iter_mut().enumerate() {
            let shift = 7 * (len - 2 - k);
            let group = ((self.number >> shift) & 0x7F) as u8;
            *slot = if k == len - 2 { group } else { group | 0x80 };
        }
        Ok(len)
    }
}
