//! OCTET STRING contents and the constructed form. X.690 §8.7, §10.2.
//!
//! Every function takes CONTENTS octets, so offsets count from the start of the contents. The caller (the TLV
//! layer) has already decided primitive or constructed and checks that each segment of a constructed encoding
//! has the universal tag 4 (X.690 §8.7.3.2 NOTE 2); this module does not.

use crate::{Error, ErrorClass, Mode};

/// True when a constructed OCTET STRING is acceptable in `mode`: BER yes (X.690 §8.7.1), DER no (X.690 §10.2).
#[must_use]
pub const fn constructed_allowed(mode: Mode) -> bool {
    !mode.is_der()
}

/// Refuse a constructed OCTET STRING in DER with `constructed_string` at `offset`. X.690 §10.2.
///
/// # Errors
/// `constructed_string` when `mode` is DER.
pub const fn check_constructed(mode: Mode, offset: usize) -> Result<(), Error> {
    if constructed_allowed(mode) {
        Ok(())
    } else {
        Err(Error::new(ErrorClass::ConstructedString, offset))
    }
}

/// The value of a primitive OCTET STRING: the contents octets themselves, zero or more. X.690 §8.7.2.
/// No content can break the rule, and BER and DER agree, so this takes no mode and cannot fail.
#[must_use]
pub const fn decode(content: &[u8]) -> &[u8] {
    content
}

/// Encode `value` as primitive contents by copying it into `out`; returns the octets written. X.690 §8.7.2.
///
/// # Errors
/// `limit_exceeded` when `out` is shorter than `value` (a buffer too small is a caller-declared limit, a
/// POLICY of this crate).
pub fn encode(value: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let out_len = out.len();
    let Some(dest) = out.get_mut(..value.len()) else {
        return Err(Error::new(ErrorClass::LimitExceeded, out_len));
    };
    dest.copy_from_slice(value);
    Ok(value.len())
}

/// A validated constructed OCTET STRING: the segments' octets, measured without allocating. BER only.
/// X.690 §8.7.3, §8.7.3.1, §8.7.3.2.
///
/// CONTRACT: the caller supplies the segments in order as an iterator of slices, each the CONTENTS of one
/// primitive OCTET STRING segment. The caller flattens nesting (§8.7.3.2 NOTE 1: a segment may itself be
/// constructed) by yielding the primitive leaves in value order, and checks the inner tags. The iterator must
/// be cheap to clone: it is walked once to measure and again each time the data is read. Segments may be empty
/// (§8.7.3.1 NOTE) and there may be none (§8.7.3 ¶1), which gives the empty value. Segment boundaries carry no
/// meaning (§8.7.3.1 ¶3).
#[derive(Clone, Debug)]
pub struct ConstructedOctetString<I> {
    segments: I,
    len: usize,
}

impl<'a, I> ConstructedOctetString<I>
where
    I: Iterator<Item = &'a [u8]> + Clone,
{
    /// Measure the segments. X.690 §8.7.3.
    ///
    /// # Errors
    /// `constructed_string` (offset 0) in DER (X.690 §10.2); `limit_exceeded` (offset the segment index) if
    /// the total does not fit a `usize`.
    pub fn new(segments: I, mode: Mode) -> Result<Self, Error> {
        check_constructed(mode, 0)?;
        let mut len = 0usize;
        for (index, segment) in segments.clone().enumerate() {
            len = len
                .checked_add(segment.len())
                .ok_or(Error::new(ErrorClass::LimitExceeded, index))?;
        }
        Ok(Self { segments, len })
    }

    /// The total number of octets. X.690 §8.7.3.1 ¶2.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// True for the empty value.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The octets of all segments in order. X.690 §8.7.3.2 ¶3.
    pub fn octets(&self) -> impl Iterator<Item = u8> {
        self.segments.clone().flat_map(|s| s.iter().copied())
    }

    /// Reassemble the octets into `out`; returns the octets written. X.690 §8.7.3.2.
    ///
    /// # Errors
    /// `limit_exceeded` when `out` is shorter than [`ConstructedOctetString::len`].
    pub fn copy_into(&self, out: &mut [u8]) -> Result<usize, Error> {
        if out.len() < self.len {
            return Err(Error::new(ErrorClass::LimitExceeded, out.len()));
        }
        for (slot, octet) in out.iter_mut().zip(self.octets()) {
            *slot = octet;
        }
        Ok(self.len)
    }
}
