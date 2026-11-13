//! BIT STRING contents: unused-bit rules, the DER rules and the constructed form. X.690 §8.6, §10.2, §11.2.
//!
//! Every function takes CONTENTS octets (no identifier, no length), so offsets count from the start of the
//! contents. The caller (the TLV layer) has already decided primitive or constructed, and has checked that each
//! segment of a constructed encoding carries the universal tag 3 (X.690 §8.6.4.1 NOTE 2); this module does not.
//!
//! Classes used (`corpus/format.md`): `bad_contents` for a break of §8.6.2, `unused_bits_nonzero` and
//! `bitstring_trailing_zeros` for the DER rules, `constructed_string` for a constructed encoding in DER, and
//! `limit_exceeded` for a total length that does not fit a `usize` or an output buffer that is too small.

use crate::{Error, ErrorClass, Mode};

/// True when a constructed BIT STRING is acceptable in `mode`: BER yes (X.690 §8.6.1), DER no (X.690 §10.2).
#[must_use]
pub const fn constructed_allowed(mode: Mode) -> bool {
    !mode.is_der()
}

/// Refuse a constructed BIT STRING in DER with `constructed_string` at `offset`. X.690 §10.2.
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

/// The mask of the used bits of a final octet with `unused` unused low bits. X.690 §8.6.2.1, §8.6.2.2.
const fn used_mask(unused: u8) -> u8 {
    0xFF << unused
}

/// Split primitive contents into the unused-bit count and the data octets. X.690 §8.6.2.
///
/// Empty contents (no initial octet, §8.6.2 "shall contain an initial octet"), an initial octet above 7
/// (§8.6.2.2 ¶2), and a non-zero initial octet with no data octets (no final octet exists to hold unused
/// bits; derived from §8.6.2.2 ¶1 with §8.6.2.3) are all `bad_contents` at offset 0.
fn split(content: &[u8]) -> Result<(u8, &[u8]), Error> {
    let Some((&unused, data)) = content.split_first() else {
        return Err(Error::new(ErrorClass::BadContents, 0));
    };
    if unused > 7 || (data.is_empty() && unused != 0) {
        return Err(Error::new(ErrorClass::BadContents, 0));
    }
    Ok((unused, data))
}

/// A decoded primitive BIT STRING, borrowed from the contents. X.690 §8.6.2.
///
/// The bit length is eight times the number of data octets minus the unused bits; the initial octet is not
/// counted. Equality compares the VALUE: the bit length and the used bits, ignoring the unused bits. POLICY:
/// BER does not constrain the values of the unused bits (no rule was found in X.690 and the standard is silent
/// on equality; docs/design/primitives.md section 4 items 4 and 9), so two BER encodings that differ only in
/// pad bits are the same value.
#[derive(Clone, Copy, Debug)]
pub struct BitString<'a> {
    data: &'a [u8],
    unused: u8,
}

impl<'a> BitString<'a> {
    /// Decode the contents of a primitive BIT STRING. X.690 §8.6.2, §11.2.
    ///
    /// `named_bit_list` says the type was defined with a `NamedBitList` (X.680 §22.7). The contents cannot show
    /// that, so the caller sets it. It matters only in DER, where all trailing 0 bits must be removed
    /// (X.690 §11.2.2) and the empty value is `00` (a single initial octet, §11.2.2 NOTE 2). In BER it is
    /// ignored: X.680 §22.7 lets encoders add or remove trailing 0 bits freely.
    ///
    /// # Errors
    /// `bad_contents` (offset 0) for empty contents, an initial octet above 7, or a non-zero initial octet with
    /// no data octets. In DER: `unused_bits_nonzero` (offset of the final data octet) when an unused bit is 1
    /// (X.690 §11.2.1), and, only with `named_bit_list`, `bitstring_trailing_zeros` (same offset) when the
    /// last bit is 0 (X.690 §11.2.2).
    pub fn decode(content: &'a [u8], mode: Mode, named_bit_list: bool) -> Result<Self, Error> {
        let (unused, data) = split(content)?;
        let value = Self { data, unused };
        if mode.is_der() {
            if let Some(&last) = data.last() {
                let at = data.len();
                if last & !used_mask(unused) != 0 {
                    return Err(Error::new(ErrorClass::UnusedBitsNonzero, at));
                }
                if named_bit_list && (last >> unused) & 1 == 0 {
                    return Err(Error::new(ErrorClass::BitstringTrailingZeros, at));
                }
            }
        }
        Ok(value)
    }

    /// The number of bits in the value. X.690 §8.6.2.1, §8.6.2.2.
    #[must_use]
    pub const fn bit_len(&self) -> usize {
        self.data.len() * 8 - self.unused as usize
    }

    /// True for the empty value (contents `00`). X.690 §8.6.2.3.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The number of unused bits in the final data octet, 0 to 7. X.690 §8.6.2.2.
    #[must_use]
    pub const fn unused_bits(&self) -> u8 {
        self.unused
    }

    /// The data octets as received, without the initial octet. The unused bits of the final octet are
    /// returned as found (BER does not constrain them); use [`BitString::bit`] or [`BitString::bits`] for the
    /// value. X.690 §8.6.2.1.
    #[must_use]
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Bit `index`, counting from the leading bit (bit 8 of the first data octet); `None` past the end.
    /// X.690 §8.6.2.1.
    #[must_use]
    pub fn bit(&self, index: usize) -> Option<bool> {
        if index >= self.bit_len() {
            return None;
        }
        Some((self.data[index / 8] >> (7 - index % 8)) & 1 == 1)
    }

    /// The bits in order, leading bit first, exactly [`BitString::bit_len`] of them. X.690 §8.6.2.1.
    pub fn bits(&self) -> impl Iterator<Item = bool> + '_ {
        (0..self.bit_len()).map(|i| (self.data[i / 8] >> (7 - i % 8)) & 1 == 1)
    }
}

impl PartialEq for BitString<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.bit_len() == other.bit_len() && self.bits().eq(other.bits())
    }
}

impl Eq for BitString<'_> {}

/// The bit length of `bits` (of which the first `bit_len` bits are the value) with all trailing 0 bits
/// removed: the length a DER encoder gives a value of a type with a `NamedBitList`. X.690 §11.2.2, X.680 §22.7.
/// A value with no 1 bits gives 0 (§11.2.2 NOTE 2). Bits past `bit_len` and past the end of `bits` are ignored.
#[must_use]
pub fn named_bit_list_len(bits: &[u8], bit_len: usize) -> usize {
    let mut len = bit_len.min(bits.len().saturating_mul(8));
    while len > 0 {
        let i = len - 1;
        if (bits[i / 8] >> (7 - i % 8)) & 1 == 1 {
            break;
        }
        len -= 1;
    }
    len
}

/// Encode the first `bit_len` bits of `bits` as primitive contents into `out`; returns the octets written.
/// The unused bits are always written as zero, whatever `bits` holds past `bit_len`, so the result satisfies
/// X.690 §11.2.1 and is valid in BER and DER. The empty value is the single octet `00` (§8.6.2.3).
/// This does not remove trailing zeros: for a `NamedBitList` type pass [`named_bit_list_len`] as `bit_len`.
///
/// # Errors
/// `bad_contents` when `bits` holds fewer than `bit_len` bits; `limit_exceeded` when `out` is shorter than
/// the contents (a buffer too small is a caller-declared limit, a POLICY of this crate).
pub fn encode(bits: &[u8], bit_len: usize, out: &mut [u8]) -> Result<usize, Error> {
    let n = bit_len.div_ceil(8);
    if bits.len() < n {
        return Err(Error::new(ErrorClass::BadContents, 0));
    }
    if out.len() < n + 1 {
        return Err(Error::new(ErrorClass::LimitExceeded, out.len()));
    }
    // n * 8 - bit_len is below 8, so the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    let unused = (n * 8 - bit_len) as u8;
    out[0] = unused;
    out[1..=n].copy_from_slice(&bits[..n]);
    if n > 0 {
        out[n] &= used_mask(unused);
    }
    Ok(n + 1)
}

/// A validated constructed BIT STRING: the segments' contents, measured without allocating. BER only.
/// X.690 §8.6.3, §8.6.4, §8.6.4.1.
///
/// CONTRACT: the caller supplies the segments in order as an iterator of slices, each the CONTENTS of one
/// primitive BIT STRING segment (initial octet, then data). The caller flattens nesting (§8.6.4.1 NOTE 1:
/// a segment may itself be constructed) by yielding the primitive leaves in value order, and checks that every
/// segment has the universal tag 3 (§8.6.4.1 NOTE 2). The iterator must be cheap to clone: it is walked once to
/// validate and again each time the data is read. Flattening is equivalent to the recursive rule: a leaf that
/// is not last in the flat order is either not last at its own level or ends a non-final constructed segment,
/// and in both cases §8.6.4 ¶2 needs a multiple of 8 bits, which for a leaf means zero unused bits.
///
/// Every segment except the last must have zero unused bits (§8.6.4 ¶2). An empty segment `00` is allowed
/// (§8.6.4 NOTE), and so are zero segments (§8.6.3), which give the empty value. The value's unused bits are
/// those of the last segment, which BER does not constrain (see [`BitString`]).
#[derive(Clone, Debug)]
pub struct ConstructedBitString<I> {
    segments: I,
    bit_len: usize,
    octets: usize,
    unused: u8,
}

impl<'a, I> ConstructedBitString<I>
where
    I: Iterator<Item = &'a [u8]> + Clone,
{
    /// Validate and measure the segments. X.690 §8.6.3, §8.6.4.
    ///
    /// # Errors
    /// `constructed_string` (offset 0) in DER (X.690 §10.2). Otherwise `bad_contents` for a segment that is
    /// empty, has an initial octet above 7, has a non-zero initial octet and no data octets, or is not the
    /// last and has unused bits; `limit_exceeded` if the total does not fit a `usize`. The offset of these
    /// errors is the INDEX of the offending segment, since the octets are not seen here.
    pub fn new(segments: I, mode: Mode) -> Result<Self, Error> {
        check_constructed(mode, 0)?;
        let mut octets = 0usize;
        let mut unused = 0u8;
        let mut it = segments.clone().enumerate().peekable();
        while let Some((index, segment)) = it.next() {
            let (u, data) = split(segment).map_err(|e| Error::new(e.class, index))?;
            if it.peek().is_some() && u != 0 {
                return Err(Error::new(ErrorClass::BadContents, index));
            }
            unused = u;
            octets = octets
                .checked_add(data.len())
                .ok_or(Error::new(ErrorClass::LimitExceeded, index))?;
        }
        let bit_len = octets
            .checked_mul(8)
            .ok_or(Error::new(ErrorClass::LimitExceeded, 0))?
            - usize::from(unused);
        Ok(Self {
            segments,
            bit_len,
            octets,
            unused,
        })
    }

    /// The total number of bits. X.690 §8.6.4 ¶3.
    #[must_use]
    pub const fn bit_len(&self) -> usize {
        self.bit_len
    }

    /// The total number of data octets (initial octets not counted).
    #[must_use]
    pub const fn octet_len(&self) -> usize {
        self.octets
    }

    /// The unused bits of the assembled value's final octet, from the last segment.
    #[must_use]
    pub const fn unused_bits(&self) -> u8 {
        self.unused
    }

    /// The data octets of all segments in order, as supplied (the unused bits of the last octet are not
    /// masked). X.690 §8.6.4.1 ¶3.
    pub fn data_octets(&self) -> impl Iterator<Item = u8> {
        self.segments
            .clone()
            .flat_map(|s| s.split_first().map_or(&[][..], |(_, d)| d).iter().copied())
    }

    /// The bits of the assembled value in order, exactly [`ConstructedBitString::bit_len`] of them.
    pub fn bits(&self) -> impl Iterator<Item = bool> {
        self.data_octets()
            .flat_map(|o| (0..8).map(move |k| (o >> (7 - k)) & 1 == 1))
            .take(self.bit_len)
    }

    /// Reassemble the data octets into `out`, with the unused bits of the final octet zeroed; returns the
    /// octets written (the primitive form's data without the initial octet). X.690 §8.6.4.1.
    ///
    /// # Errors
    /// `limit_exceeded` when `out` is shorter than [`ConstructedBitString::octet_len`].
    pub fn copy_data(&self, out: &mut [u8]) -> Result<usize, Error> {
        if out.len() < self.octets {
            return Err(Error::new(ErrorClass::LimitExceeded, out.len()));
        }
        for (slot, octet) in out.iter_mut().zip(self.data_octets()) {
            *slot = octet;
        }
        if self.octets > 0 {
            out[self.octets - 1] &= used_mask(self.unused);
        }
        Ok(self.octets)
    }
}
