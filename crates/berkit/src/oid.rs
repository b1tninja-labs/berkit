//! OBJECT IDENTIFIER, RELATIVE-OID, OID-IRI and RELATIVE-OID-IRI contents. X.690 §8.19 to §8.22.
//!
//! Every function here works on the CONTENTS octets of one value; the identifier and length octets belong to
//! the framing module. The identifier octets are `06` (OBJECT IDENTIFIER) and `0D` (RELATIVE-OID). OID-IRI
//! and RELATIVE-OID-IRI are universal tags 35 and 36, which do not fit the single-octet identifier form, so
//! their identifier octets are the two octets `1F 23` and `1F 24` (X.690 §8.1.2.4), not `23` or `24` (those are
//! the constructed BIT STRING and OCTET STRING).
//!
//! ## Arcs have no size limit
//!
//! X.690 §8.19.2 states no maximum for a subidentifier, and arcs of 128 bits are in use (`2.25.<UUID>`).
//! The decoders therefore never convert an arc themselves: [`Arc`] borrows the subidentifier octets and offers
//! [`Arc::to_u64`] and [`Arc::to_u128`], which return `None` on overflow and never truncate, and
//! [`Arc::be_bytes`], which gives the exact value at any size.
//!
//! ## Mode
//!
//! X.690 §9 and §11 add no restriction for these four types, so BER and DER accept and refuse the same
//! contents. The `mode` argument is kept so that every content function has the same shape.
//!
//! ## Policies (the standard is silent or the source is not in the corpus)
//!
//! * POLICY (derived): empty contents of an OBJECT IDENTIFIER (no first subidentifier, X.690 §8.19.3 with
//!   the "at least two arcs" NOTE of X.680 §32.11) and of a RELATIVE-OID (X.680 §33.3 has at least one
//!   component) is refused as `bad_contents`.
//! * POLICY: a subidentifier whose last octet is missing (the contents end while bit 8 is still 1) is
//!   `bad_contents`, not `truncated`: the corpus defines `truncated` as the input ending inside the
//!   identifier, length or declared contents, and here the declared contents are complete.
//! * POLICY: the encoder refuses a first arc other than 0, 1 or 2, and a second arc above 39 when the first
//!   is 0 or 1. The rule is X.660 §7.6 ¶1 (a NOTE in X.690 §8.19.4 repeats it). The decoder applies the
//!   split of §8.19.4 and accepts any value.

use core::fmt;

use crate::{Error, ErrorClass, Mode};

// --------------------------------------------------------------------------------------------------------
// Subidentifier helpers
// --------------------------------------------------------------------------------------------------------

/// Number of significant bits of the unsigned number a validated subidentifier encodes.
fn sub_bits(octets: &[u8]) -> usize {
    let first = octets[0] & 0x7F;
    if first == 0 {
        return 0; // only the single octet `00`: the value zero
    }
    (octets.len() - 1) * 7 + (8 - first.leading_zeros() as usize)
}

/// Value of a validated subidentifier minus `bias`, or `None` if that does not fit in 128 bits.
///
/// The accumulator wraps; the bit count says whether anything was lost, so nothing is truncated silently.
fn magnitude_u128(octets: &[u8], bias: u8) -> Option<u128> {
    let mut v: u128 = 0;
    for &b in octets {
        v = (v << 7) | u128::from(b & 0x7F);
    }
    let width = sub_bits(octets);
    if width <= 128 {
        // `bias` is 0 or 80 and the subidentifier is at least `bias`, so this cannot wrap.
        Some(v.wrapping_sub(u128::from(bias)))
    } else if width == 129 && v < u128::from(bias) {
        // The subidentifier is 2**128 + v, so subtracting `bias` still fits when v is below it.
        Some(v.wrapping_sub(u128::from(bias)))
    } else {
        None
    }
}

/// The low octet of `x`; the discarded high part is intended.
fn low(x: u16) -> u8 {
    x.to_le_bytes()[0]
}

/// Splits the next subidentifier off `rest`. The contents were validated, so the end octet exists.
fn take_subidentifier<'a>(rest: &mut &'a [u8]) -> Option<&'a [u8]> {
    let end = rest.iter().position(|&b| b < 0x80)? + 1;
    let (sub, tail) = rest.split_at(end);
    *rest = tail;
    Some(sub)
}

/// Checks that `content` is a list of minimal subidentifiers (X.690 §8.19.2, §8.20.2).
fn validate_subidentifiers(content: &[u8]) -> Result<(), Error> {
    if content.is_empty() {
        return Err(Error::new(ErrorClass::BadContents, 0));
    }
    let mut start = 0;
    while start < content.len() {
        if content[start] == 0x80 {
            return Err(Error::new(ErrorClass::NonMinimalSubidentifier, start));
        }
        match content[start..].iter().position(|&b| b < 0x80) {
            Some(n) => start += n + 1,
            None => return Err(Error::new(ErrorClass::BadContents, start)),
        }
    }
    Ok(())
}

// --------------------------------------------------------------------------------------------------------
// Decoding
// --------------------------------------------------------------------------------------------------------

/// One arc of an OBJECT IDENTIFIER or RELATIVE-OID, borrowed from the contents. X.690 §8.19.2, §8.19.4.
///
/// It is not converted at decode time. The octets are those of the subidentifier the arc was read from; for
/// the first two arcs of an OBJECT IDENTIFIER both arcs report the same first subidentifier (§8.19.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arc<'a> {
    octets: &'a [u8],
    /// The value when it was split off the first subidentifier and is below 80; otherwise `None`.
    small: Option<u8>,
    /// Subtracted from the subidentifier when `small` is `None` (80 for the second arc under arc 2).
    bias: u8,
}

impl<'a> Arc<'a> {
    fn plain(octets: &'a [u8]) -> Self {
        Self {
            octets,
            small: None,
            bias: 0,
        }
    }

    /// The octets of the subidentifier this arc was read from. X.690 §8.19.2.
    #[must_use]
    pub const fn octets(&self) -> &'a [u8] {
        self.octets
    }

    /// The arc as a `u128`, or `None` if it does not fit. Never truncates. X.690 §8.19.2 ¶5.
    #[must_use]
    pub fn to_u128(&self) -> Option<u128> {
        match self.small {
            Some(v) => Some(u128::from(v)),
            None => magnitude_u128(self.octets, self.bias),
        }
    }

    /// The arc as a `u64`, or `None` if it does not fit. Never truncates. X.690 §8.19.2 ¶5.
    #[must_use]
    pub fn to_u64(&self) -> Option<u64> {
        self.to_u128().and_then(|v| u64::try_from(v).ok())
    }

    /// Writes the exact value of the arc into `out` as the fewest big-endian octets (zero is one octet `00`)
    /// and returns how many were written, or `None` if `out` is too small. An `out` as long as
    /// [`Arc::octets`] always suffices. X.690 §8.19.2 ¶5.
    #[must_use]
    pub fn be_bytes(&self, out: &mut [u8]) -> Option<usize> {
        if let Some(v) = self.small {
            *out.first_mut()? = v;
            return Some(1);
        }
        let bits = sub_bits(self.octets);
        let nb = bits.div_ceil(8).max(1);
        let buf = out.get_mut(..nb)?;
        buf.fill(0);
        for (i, &b) in self.octets.iter().rev().enumerate() {
            let g = u16::from(b & 0x7F);
            let p = 7 * i;
            let s = p % 8;
            let idx = nb - 1 - p / 8;
            buf[idx] |= low(g << s);
            if s > 1 && idx > 0 {
                buf[idx - 1] |= low(g >> (8 - s));
            }
        }
        let mut borrow = u16::from(self.bias);
        for byte in buf.iter_mut().rev() {
            if borrow == 0 {
                break;
            }
            let cur = u16::from(*byte);
            if cur >= borrow {
                *byte = low(cur - borrow);
                borrow = 0;
            } else {
                *byte = low(cur + 256 - borrow);
                borrow = 1;
            }
        }
        let lead = buf.iter().position(|&b| b != 0).unwrap_or(nb - 1);
        buf.copy_within(lead.., 0);
        Some(nb - lead)
    }
}

/// Iterator over the arcs of an [`ObjectIdentifier`]. X.690 §8.19.4, §8.19.5.
#[derive(Clone, Debug)]
pub struct ObjectIdentifierArcs<'a> {
    rest: &'a [u8],
    pending: Option<Arc<'a>>,
    first: bool,
}

impl<'a> Iterator for ObjectIdentifierArcs<'a> {
    type Item = Arc<'a>;

    fn next(&mut self) -> Option<Arc<'a>> {
        if let Some(second) = self.pending.take() {
            return Some(second);
        }
        let sub = take_subidentifier(&mut self.rest)?;
        if !self.first {
            return Some(Arc::plain(sub));
        }
        self.first = false;
        // X.690 §8.19.4: first subidentifier = X*40 + Y. A value of 128 or more is above 80.
        let single = if sub.len() == 1 { Some(sub[0]) } else { None };
        let (x, small, bias) = match single {
            Some(v) if v < 40 => (0, Some(v), 0),
            Some(v) if v < 80 => (1, Some(v - 40), 0),
            _ => (2, None, 80),
        };
        self.pending = Some(Arc {
            octets: sub,
            small,
            bias,
        });
        Some(Arc {
            octets: sub,
            small: Some(x),
            bias: 0,
        })
    }
}

/// An OBJECT IDENTIFIER value: a validated view over its contents octets. X.690 §8.19.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectIdentifier<'a> {
    content: &'a [u8],
}

impl<'a> ObjectIdentifier<'a> {
    /// Validates the contents of an OBJECT IDENTIFIER and returns the view. X.690 §8.19.2.
    ///
    /// BER and DER are the same here. Offsets count from the start of `content`.
    ///
    /// # Errors
    ///
    /// * `non_minimal_subidentifier` if a subidentifier starts with octet `80` (§8.19.2 ¶6).
    /// * `bad_contents` if the last subidentifier never ends (§8.19.2 ¶3) or the contents are empty
    ///   (POLICY, derived: there is no first subidentifier, §8.19.3, §8.19.4).
    pub fn decode(content: &'a [u8], _mode: Mode) -> Result<Self, Error> {
        validate_subidentifiers(content)?;
        Ok(Self { content })
    }

    /// The contents octets.
    #[must_use]
    pub const fn contents(&self) -> &'a [u8] {
        self.content
    }

    /// The arcs in order. The first subidentifier yields two arcs (§8.19.4): below 40 the first arc is 0,
    /// below 80 it is 1, otherwise 2 and the second arc is the value minus 80, of any size.
    #[must_use]
    pub fn arcs(&self) -> ObjectIdentifierArcs<'a> {
        ObjectIdentifierArcs {
            rest: self.content,
            pending: None,
            first: true,
        }
    }
}

/// Iterator over the arcs of a [`RelativeOid`]. X.690 §8.20.4.
#[derive(Clone, Debug)]
pub struct RelativeOidArcs<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for RelativeOidArcs<'a> {
    type Item = Arc<'a>;

    fn next(&mut self) -> Option<Arc<'a>> {
        take_subidentifier(&mut self.rest).map(Arc::plain)
    }
}

/// A RELATIVE-OID value: a validated view over its contents octets. X.690 §8.20.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelativeOid<'a> {
    content: &'a [u8],
}

impl<'a> RelativeOid<'a> {
    /// Validates the contents of a RELATIVE-OID and returns the view. X.690 §8.20.2.
    ///
    /// Same rules as [`ObjectIdentifier::decode`], with no packing of the first two arcs: every
    /// sub-identifier is one arc (§8.20.3, §8.20.4).
    ///
    /// # Errors
    ///
    /// `non_minimal_subidentifier` for a leading `80` (§8.20.2 ¶6); `bad_contents` for an unterminated last
    /// sub-identifier (§8.20.2 ¶3) or empty contents (POLICY, derived from X.680 §33.3).
    pub fn decode(content: &'a [u8], _mode: Mode) -> Result<Self, Error> {
        validate_subidentifiers(content)?;
        Ok(Self { content })
    }

    /// The contents octets.
    #[must_use]
    pub const fn contents(&self) -> &'a [u8] {
        self.content
    }

    /// The arcs in order, one per sub-identifier.
    #[must_use]
    pub fn arcs(&self) -> RelativeOidArcs<'a> {
        RelativeOidArcs { rest: self.content }
    }
}

// --------------------------------------------------------------------------------------------------------
// Encoding
// --------------------------------------------------------------------------------------------------------

/// Why an encoder refused its input. These are caller errors, not corpus error classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The output slice is too short.
    BufferTooSmall,
    /// An OBJECT IDENTIFIER needs two arcs (X.690 §8.19.4) and a RELATIVE-OID one (§8.20.3).
    TooFewArcs,
    /// First arc not 0, 1 or 2. X.660 §7.6 ¶1 a); a NOTE in X.690 §8.19.4 repeats it.
    FirstArcOutOfRange,
    /// Second arc above 39 under first arc 0 or 1. X.660 §7.6 ¶1 b).
    SecondArcOutOfRange,
    /// The text is not valid contents of an OID-IRI or RELATIVE-OID-IRI (see [`OidIri::decode`]).
    InvalidIri,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BufferTooSmall => "output buffer too small",
            Self::TooFewArcs => "too few arcs",
            Self::FirstArcOutOfRange => "first arc is not 0, 1 or 2",
            Self::SecondArcOutOfRange => "second arc above 39 under first arc 0 or 1",
            Self::InvalidIri => "not a valid OID-IRI text",
        })
    }
}

impl core::error::Error for EncodeError {}

/// A source of an arc value as big-endian octets. Private: the public functions name the three forms.
trait ArcSource {
    fn with_be<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R;
}

impl ArcSource for u64 {
    fn with_be<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        f(&self.to_be_bytes())
    }
}

impl ArcSource for u128 {
    fn with_be<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        f(&self.to_be_bytes())
    }
}

impl ArcSource for &[u8] {
    fn with_be<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        f(self)
    }
}

fn strip_leading_zeros(m: &[u8]) -> &[u8] {
    let lead = m.iter().position(|&b| b != 0).unwrap_or(m.len());
    &m[lead..]
}

/// Bits `7*i .. 7*i+7` of the big-endian number `m`, counted from the least significant bit.
fn group(m: &[u8], i: usize) -> u16 {
    let p = 7 * i;
    let s = p % 8;
    let from_end = p / 8;
    let get = |k: usize| -> u16 {
        if k < m.len() {
            u16::from(m[m.len() - 1 - k])
        } else {
            0
        }
    };
    ((get(from_end) >> s) | (get(from_end + 1) << (8 - s))) & 0x7F
}

/// Writes the subidentifier for `m + add` (X.690 §8.19.2) and returns its length. `m` is big-endian.
fn put_subidentifier(m: &[u8], add: u8, out: &mut [u8]) -> Result<usize, EncodeError> {
    let m = strip_leading_zeros(m);
    let bits = m
        .first()
        .map_or(0, |&b| (m.len() - 1) * 8 + (8 - b.leading_zeros() as usize));
    let groups = bits.div_ceil(7).max(1);
    let mut carry = u16::from(add);
    for i in 0..groups {
        carry = (group(m, i) + carry) >> 7;
    }
    let total = groups + usize::from(carry != 0);
    let out = out.get_mut(..total).ok_or(EncodeError::BufferTooSmall)?;
    let mut carry = u16::from(add);
    for i in 0..total {
        let t = group(m, i) + carry;
        carry = t >> 7;
        let continuation = if i == 0 { 0 } else { 0x80 };
        out[total - 1 - i] = low(t & 0x7F) | continuation;
    }
    Ok(total)
}

fn encode_list<A: ArcSource>(
    arcs: &[A],
    relative: bool,
    out: &mut [u8],
) -> Result<usize, EncodeError> {
    let out_len = out.len();
    let mut len = 0;
    let mut rest = arcs;
    if !relative {
        let [first, second, tail @ ..] = arcs else {
            return Err(EncodeError::TooFewArcs);
        };
        let x = first.with_be(|m| {
            let m = strip_leading_zeros(m);
            match m {
                [] => Some(0u8),
                [v @ 0..=2] => Some(*v),
                _ => None,
            }
        });
        let x = x.ok_or(EncodeError::FirstArcOutOfRange)?;
        len = second.with_be(|m| {
            if x == 2 {
                // X.690 §8.19.4: 2*40 + Y, with Y of any size.
                put_subidentifier(m, 80, out)
            } else {
                match strip_leading_zeros(m) {
                    [] => put_subidentifier(&[x * 40], 0, out),
                    [y @ 0..=39] => put_subidentifier(&[x * 40 + y], 0, out),
                    _ => Err(EncodeError::SecondArcOutOfRange),
                }
            }
        })?;
        rest = tail;
    } else if arcs.is_empty() {
        return Err(EncodeError::TooFewArcs);
    }
    for arc in rest {
        let tail = &mut out[len.min(out_len)..];
        len += arc.with_be(|m| put_subidentifier(m, 0, tail))?;
    }
    Ok(len)
}

/// Writes the contents of an OBJECT IDENTIFIER from `u64` arcs into `out`; returns the length. X.690 §8.19.
///
/// The first two arcs become one subidentifier, `X*40 + Y` (§8.19.4). Every subidentifier is minimal.
///
/// # Errors
///
/// [`EncodeError::TooFewArcs`] below two arcs, [`EncodeError::FirstArcOutOfRange`] and
/// [`EncodeError::SecondArcOutOfRange`] (X.660 §7.6 ¶1), [`EncodeError::BufferTooSmall`].
pub fn encode_object_identifier_u64(arcs: &[u64], out: &mut [u8]) -> Result<usize, EncodeError> {
    encode_list(arcs, false, out)
}

/// As [`encode_object_identifier_u64`] with `u128` arcs, enough for `2.25.<UUID>`. X.690 §8.19.
///
/// # Errors
///
/// As [`encode_object_identifier_u64`].
pub fn encode_object_identifier_u128(arcs: &[u128], out: &mut [u8]) -> Result<usize, EncodeError> {
    encode_list(arcs, false, out)
}

/// As [`encode_object_identifier_u64`] with each arc a big-endian unsigned magnitude of any length. Leading
/// zero octets are ignored and an empty slice is zero. X.690 §8.19.
///
/// # Errors
///
/// As [`encode_object_identifier_u64`].
pub fn encode_object_identifier_be(arcs: &[&[u8]], out: &mut [u8]) -> Result<usize, EncodeError> {
    encode_list(arcs, false, out)
}

/// Writes the contents of a RELATIVE-OID from `u64` arcs; returns the length. X.690 §8.20.
///
/// # Errors
///
/// [`EncodeError::TooFewArcs`] for no arcs (POLICY, derived from X.680 §33.3), [`EncodeError::BufferTooSmall`].
pub fn encode_relative_oid_u64(arcs: &[u64], out: &mut [u8]) -> Result<usize, EncodeError> {
    encode_list(arcs, true, out)
}

/// As [`encode_relative_oid_u64`] with `u128` arcs. X.690 §8.20.
///
/// # Errors
///
/// As [`encode_relative_oid_u64`].
pub fn encode_relative_oid_u128(arcs: &[u128], out: &mut [u8]) -> Result<usize, EncodeError> {
    encode_list(arcs, true, out)
}

/// As [`encode_relative_oid_u64`] with big-endian magnitudes of any length. X.690 §8.20.
///
/// # Errors
///
/// As [`encode_relative_oid_u64`].
pub fn encode_relative_oid_be(arcs: &[&[u8]], out: &mut [u8]) -> Result<usize, EncodeError> {
    encode_list(arcs, true, out)
}

// --------------------------------------------------------------------------------------------------------
// OID-IRI and RELATIVE-OID-IRI
// --------------------------------------------------------------------------------------------------------

/// Whether `c` is in the character repertoire of a non-integer Unicode label. X.660 §7.5.2.
///
/// ASCII admits only HYPHEN-MINUS, FULL STOP, LOW LINE, TILDE, digits and letters; above that U+00A0 to
/// U+DFFE, U+F900 to U+FDCF, U+FDF0 to U+FFEF, the last two code points of no plane from 1 to 13 excluded, and
/// U+E1000 to U+EFFFD. The "(This position shall not be used)" exclusion of §7.5.3 needs ISO/IEC 10646
/// and is not applied.
const fn is_label_char(c: char) -> bool {
    let v = c as u32;
    match v {
        0x2D
        | 0x2E
        | 0x5F
        | 0x7E
        | 0x30..=0x39
        | 0x41..=0x5A
        | 0x61..=0x7A
        | 0xA0..=0xDFFE
        | 0xF900..=0xFDCF
        | 0xFDF0..=0xFFEF
        | 0xE1000..=0xEFFFD => true,
        0x1_0000..=0xD_FFFF => v & 0xFFFF <= 0xFFFD,
        _ => false,
    }
}

/// Checks the contents of an IRI value and returns them as text. `absolute` selects X.680 §34.3 (starts with
/// `/`) over §35.3 (the first arc has no slash).
fn validate_iri(content: &[u8], absolute: bool) -> Result<&str, Error> {
    let text = core::str::from_utf8(content)
        .map_err(|e| Error::new(ErrorClass::BadContents, e.valid_up_to()))?;
    let mut pos = 0;
    let mut body = text;
    if absolute {
        body = text
            .strip_prefix('/')
            .ok_or(Error::new(ErrorClass::BadContents, 0))?;
        pos = 1;
    }
    for label in body.split('/') {
        if label.is_empty() {
            return Err(Error::new(ErrorClass::BadContents, pos));
        }
        // X.660 §7.5.2 (X.680 §12.27 ¶1): every character of a label is in the label repertoire. This also
        // refuses the ASCII white-space the notation of X.680 §12.1.6 would skip.
        if let Some((i, _)) = label.char_indices().find(|&(_, c)| !is_label_char(c)) {
            return Err(Error::new(ErrorClass::BadContents, pos + i));
        }
        // X.680 §12.26 ¶2 and §12.27 ¶2: an all-digit label is an integer label, which has no leading zero.
        if label.len() > 1 && label.starts_with('0') && label.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::new(ErrorClass::BadContents, pos));
        }
        pos += label.len() + 1;
    }
    Ok(text)
}

/// Iterator over the labels of an IRI value, each a `/`-free text.
#[derive(Clone, Debug)]
pub struct IriArcs<'a>(core::str::Split<'a, char>);

impl<'a> Iterator for IriArcs<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        self.0.next()
    }
}

/// An OID-IRI value (universal tag 35, identifier octets `1F 23`): contents are the UTF-8 of `/arc/arc...`.
/// X.690 §8.21.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OidIri<'a> {
    text: &'a str,
}

impl<'a> OidIri<'a> {
    /// Validates the contents of an OID-IRI. X.690 §8.21.2, X.680 §34.3.
    ///
    /// The contents are the UTF-8 of the lexical items of `FirstArcIdentifier SubsequentArcIdentifier`
    /// (§8.21.2 ¶1): a `/` then a label, repeated, with no quotation marks (`XMLIRIValue`). Checked: well-formed
    /// UTF-8 (the shortest form, §8.21.2 ¶2; Rust's decoder also refuses surrogates and values above
    /// U+10FFFF); the leading `/`; no empty label; every character in the label repertoire of
    /// X.660 §7.5.2 (so no ASCII white-space, no control character, no private-use or noncharacter code point);
    /// an all-digit label has
    /// no leading zero (X.680 §12.26 ¶2, §12.27 ¶2). Empty contents are refused (POLICY, derived: the
    /// grammar needs at least one arc). The one rule of X.660 §7.5.3 not checked is the exclusion of ISO/IEC 10646
    /// positions marked "(This position shall not be used)", which needs 10646 (`[unread]`); the NOTE there
    /// invites tolerance.
    ///
    /// # Errors
    ///
    /// `bad_contents` at the offset of the first violation.
    pub fn decode(content: &'a [u8], _mode: Mode) -> Result<Self, Error> {
        validate_iri(content, true).map(|text| Self { text })
    }

    /// Validates `text` as for [`OidIri::decode`] and copies its UTF-8 into `out`; returns the length.
    ///
    /// # Errors
    ///
    /// [`EncodeError::InvalidIri`] or [`EncodeError::BufferTooSmall`].
    pub fn encode(text: &str, out: &mut [u8]) -> Result<usize, EncodeError> {
        validate_iri(text.as_bytes(), true).map_err(|_| EncodeError::InvalidIri)?;
        let dst = out
            .get_mut(..text.len())
            .ok_or(EncodeError::BufferTooSmall)?;
        dst.copy_from_slice(text.as_bytes());
        Ok(text.len())
    }

    /// The contents as text, including the leading `/`.
    #[must_use]
    pub const fn as_str(&self) -> &'a str {
        self.text
    }

    /// The labels in order, without the slashes.
    #[must_use]
    pub fn arcs(&self) -> IriArcs<'a> {
        IriArcs(self.text[1..].split('/'))
    }
}

/// A RELATIVE-OID-IRI value (universal tag 36, identifier octets `1F 24`): the UTF-8 of `arc/arc...` with no
/// leading slash. X.690 §8.22.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelativeOidIri<'a> {
    text: &'a str,
}

impl<'a> RelativeOidIri<'a> {
    /// Validates the contents of a RELATIVE-OID-IRI. X.690 §8.22.2, X.680 §35.3.
    ///
    /// As [`OidIri::decode`], except the first label has no leading `/`
    /// (`FirstRelativeArcIdentifier SubsequentArcIdentifier`). §8.22.2 does not repeat the shortest-form
    /// sentence of §8.21.2 ¶2; the contents are UTF-8 all the same, so the same check applies (POLICY, derived).
    ///
    /// # Errors
    ///
    /// `bad_contents` at the offset of the first violation.
    pub fn decode(content: &'a [u8], _mode: Mode) -> Result<Self, Error> {
        validate_iri(content, false).map(|text| Self { text })
    }

    /// Validates `text` as for [`RelativeOidIri::decode`] and copies its UTF-8 into `out`.
    ///
    /// # Errors
    ///
    /// [`EncodeError::InvalidIri`] or [`EncodeError::BufferTooSmall`].
    pub fn encode(text: &str, out: &mut [u8]) -> Result<usize, EncodeError> {
        validate_iri(text.as_bytes(), false).map_err(|_| EncodeError::InvalidIri)?;
        let dst = out
            .get_mut(..text.len())
            .ok_or(EncodeError::BufferTooSmall)?;
        dst.copy_from_slice(text.as_bytes());
        Ok(text.len())
    }

    /// The contents as text.
    #[must_use]
    pub const fn as_str(&self) -> &'a str {
        self.text
    }

    /// The labels in order.
    #[must_use]
    pub fn arcs(&self) -> IriArcs<'a> {
        IriArcs(self.text.split('/'))
    }
}
