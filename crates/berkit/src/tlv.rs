//! The tag-length-value reader and writer: elements that keep their exact input octets. X.690 §8.1.
//!
//! The reader is zero-copy. Every [`Tlv`] holds three slices of the input: the header (identifier and length
//! octets), the contents octets, and the whole encoding (header, contents and, for the indefinite form, the
//! end-of-contents octets). A DER SET OF comparison (X.690 §11.6) or a signature check needs those original
//! octets, not a re-encoding.
//!
//! This layer applies only the structural rules: identifier, length, end-of-contents, nesting and, in DER, the
//! framing rules of X.690 §10.1. It never looks at the type, so it does not check the constructed bit against
//! the type (`wrong_form`) and does not check contents; the typed layer does (`docs/design/structured.md` §5).
//!
//! Error order is STREAM ORDER: identifier, then length, then contents, depth first.
//!
//! # Where the input ends
//!
//! An input that stops inside an identifier, a length or declared contents is `truncated` when the input
//! itself ends there, and `length_overrun` when the cut is the end of a definite-length parent that lies
//! wholly inside the input (the child does not fit its parent, X.690 §8.1.3.3, §8.1.4). An indefinite-length
//! encoding that reaches the end of its enclosing contents exactly at an element boundary is `missing_eoc`
//! (`corpus/format.md`, X.690 §8.1.3.6.2).

use crate::length::{self, Length};
use crate::tag::{BufferTooSmall, Class, Tag};
use crate::{Error, ErrorClass, Mode};
use core::fmt;

/// The class reported for a stray end-of-contents or any other UNIVERSAL 0 identifier where no indefinite-length
/// encoding is open, and for tag 0 with more than the two EOC octets (`00 01 05`, `20 00`).
///
/// X.690 §8.1.1.2 and §8.1.5 allow the EOC octets only after an indefinite length, and X.680 §8.4 Table 1
/// reserves UNIVERSAL 0 for the encoding rules. The shared error list has the class `stray_eoc` for it.
pub const STRAY_EOC: ErrorClass = ErrorClass::StrayEoc;

/// Resource limits of the reader. The standard sets none (X.690 §8.1.2.4.2 b), §8.1.3.5), so every value here
/// is POLICY: a choice made so that hostile input cannot cost unbounded stack or accept absurd lengths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The deepest nesting accepted, counting the top-level element as level 1. An element at a deeper level
    /// is `limit_exceeded`. The reader recurses at most this deep, and never deeper than
    /// [`Limits::HARD_MAX_DEPTH`] whatever is requested here. Default 64.
    pub max_depth: usize,
    /// The largest definite contents length accepted, in octets. A longer one that fits the input is
    /// `limit_exceeded`. A length larger than the input is `truncated` or `length_overrun` first, because the
    /// octets are not there. Default 256 MiB.
    pub max_content_length: u64,
}

impl Limits {
    /// The deepest recursion the reader will ever do, whatever `max_depth` says: a stack-safety bound.
    pub const HARD_MAX_DEPTH: usize = 512;

    /// The documented defaults: depth 64, contents 256 MiB.
    pub const DEFAULT: Self = Self {
        max_depth: 64,
        max_content_length: 1 << 28,
    };

    const fn depth(&self) -> usize {
        if self.max_depth < Self::HARD_MAX_DEPTH {
            self.max_depth
        } else {
            Self::HARD_MAX_DEPTH
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// One tag-length-value element, borrowing its octets from the input. X.690 §8.1.1.1.
#[derive(Clone, Copy, Debug)]
pub struct Tlv<'a> {
    tag: Tag,
    length: Length,
    header: &'a [u8],
    content: &'a [u8],
    encoding: &'a [u8],
    offset: usize,
    level: usize,
    mode: Mode,
    limits: Limits,
}

#[derive(Clone, Copy)]
struct Ctx {
    mode: Mode,
    limits: Limits,
    /// True when the end of the region being read is the end of a definite-length parent that lies wholly
    /// inside the input, so running off it is `length_overrun`, not `truncated`.
    bounded: bool,
}

enum Item<'a> {
    Element(Tlv<'a>),
    Eoc,
}

impl Ctx {
    /// An error from a header decoder, moved to absolute offsets and, inside a bounded region, with
    /// `truncated` read as `length_overrun`.
    fn adjust(&self, e: Error, base: usize) -> Error {
        let class = if e.class == ErrorClass::Truncated && self.bounded {
            ErrorClass::LengthOverrun
        } else {
            e.class
        };
        Error::new(class, base + e.offset)
    }
}

/// Read one element, or one end-of-contents, at the start of `input`. `base` is the offset of `input[0]` in
/// the top-level input; `level` is the nesting level of the element (the top level is 1).
fn next_item<'a>(
    input: &'a [u8],
    ctx: Ctx,
    indef_open: bool,
    level: usize,
    base: usize,
) -> Result<Item<'a>, Error> {
    // End-of-contents: exactly `00 00`, and only where an indefinite-length encoding is open (X.690 §8.1.5).
    if indef_open && input.first() == Some(&0) {
        return match input.get(1) {
            None => Err(ctx.adjust(Error::new(ErrorClass::Truncated, 1), base)),
            Some(0) => Ok(Item::Eoc),
            Some(_) => Err(Error::new(STRAY_EOC, base)),
        };
    }
    // Identifier octets.
    let (tag, tag_len) = Tag::decode(input).map_err(|e| ctx.adjust(e, base))?;
    if tag.is_universal_zero() {
        return Err(Error::new(STRAY_EOC, base));
    }
    // Length octets.
    let d = length::decode_raw(&input[tag_len..]).map_err(|e| ctx.adjust(e, base + tag_len))?;
    let header_len = tag_len + d.consumed;
    match d.length {
        Length::Indefinite => {
            // Decidable from the identifier alone, so it comes before the DER rule (X.690 §8.1.3.2 a)).
            if !tag.constructed {
                return Err(Error::new(
                    ErrorClass::IndefiniteOnPrimitive,
                    base + tag_len,
                ));
            }
            if ctx.mode.is_der() {
                return Err(Error::new(ErrorClass::IndefiniteLength, base + tag_len));
            }
        }
        Length::Definite(_) => {
            if ctx.mode.is_der() && !d.minimal {
                return Err(Error::new(ErrorClass::NonMinimalLength, base + tag_len));
            }
        }
    }
    if level > ctx.limits.depth() {
        return Err(Error::new(ErrorClass::LimitExceeded, base));
    }
    let header = &input[..header_len];
    let make = |content: &'a [u8], encoding: &'a [u8]| Tlv {
        tag,
        length: d.length,
        header,
        content,
        encoding,
        offset: base,
        level,
        mode: ctx.mode,
        limits: ctx.limits,
    };
    match d.length {
        Length::Definite(n) => {
            let available = input.len() - header_len;
            if n > available as u64 {
                let class = if ctx.bounded {
                    ErrorClass::LengthOverrun
                } else {
                    ErrorClass::Truncated
                };
                return Err(Error::new(class, base + header_len));
            }
            if n > ctx.limits.max_content_length {
                return Err(Error::new(ErrorClass::LimitExceeded, base + tag_len));
            }
            // `n <= available`, so the conversion cannot lose bits.
            #[allow(clippy::cast_possible_truncation)]
            let end = header_len + n as usize;
            Ok(Item::Element(make(&input[header_len..end], &input[..end])))
        }
        Length::Indefinite => {
            // Read the children in stream order until the EOC closes this element (X.690 §8.1.3.6.2, §8.1.5).
            let mut pos = header_len;
            loop {
                let rest = &input[pos..];
                if rest.is_empty() {
                    return Err(Error::new(ErrorClass::MissingEoc, base + pos));
                }
                match next_item(rest, ctx, true, level + 1, base + pos)? {
                    Item::Eoc => {
                        return Ok(Item::Element(make(
                            &input[header_len..pos],
                            &input[..pos + 2],
                        )));
                    }
                    Item::Element(child) => {
                        // An indefinite child was validated while it was read; a definite one is walked now
                        // so that faults inside it come before any later fault.
                        if child.length != Length::Indefinite {
                            child.validate()?;
                        }
                        pos += child.encoding.len();
                    }
                }
            }
        }
    }
}

impl<'a> Tlv<'a> {
    /// Read one element from the start of `input` and return it with the octets after it. X.690 §8.1.1.1.
    ///
    /// For an indefinite length the contents are read to the end-of-contents octets, validating everything
    /// inside in stream order. For a definite length only the header and the extent are checked; call
    /// [`Tlv::validate`] (or [`Tlv::parse_exact`]) to walk the children.
    ///
    /// Octets after the element are not an error here; they are returned as the rest.
    ///
    /// # Errors
    /// Any class named in the module documentation, with offsets from the start of `input`.
    pub fn parse(input: &'a [u8], mode: Mode, limits: &Limits) -> Result<(Self, &'a [u8]), Error> {
        let ctx = Ctx {
            mode,
            limits: *limits,
            bounded: false,
        };
        match next_item(input, ctx, false, 1, 0)? {
            Item::Element(t) => {
                let rest = &input[t.encoding.len()..];
                Ok((t, rest))
            }
            // Unreachable: no indefinite encoding is open, so tag 0 is refused before it becomes an Eoc.
            Item::Eoc => Err(Error::new(STRAY_EOC, 0)),
        }
    }

    /// Read exactly one element from `input` and validate the whole tree, depth first. Octets left after it
    /// are `trailing_data`, reported after any fault inside the element (stream order). X.690 §8.1.1.1.
    ///
    /// # Errors
    /// Any class named in the module documentation, and `trailing_data` (a policy of this crate).
    pub fn parse_exact(input: &'a [u8], mode: Mode, limits: &Limits) -> Result<Self, Error> {
        let (tlv, rest) = Self::parse(input, mode, limits)?;
        tlv.validate()?;
        if rest.is_empty() {
            Ok(tlv)
        } else {
            Err(Error::new(ErrorClass::TrailingData, tlv.encoding.len()))
        }
    }

    /// Check every descendant, depth first, in stream order. Recursion is bounded by [`Limits`].
    ///
    /// # Errors
    /// The first fault in stream order.
    pub fn validate(&self) -> Result<(), Error> {
        // An indefinite-length element was walked in full when it was read.
        if self.length == Length::Indefinite {
            return Ok(());
        }
        for child in self.children() {
            child?.validate()?;
        }
        Ok(())
    }

    /// The identifier octets, decoded. X.690 §8.1.2.
    #[must_use]
    pub const fn tag(&self) -> Tag {
        self.tag
    }

    /// The length as written: a definite number of octets or the indefinite form. X.690 §8.1.3.
    #[must_use]
    pub const fn length(&self) -> Length {
        self.length
    }

    /// True when bit 6 of the identifier is 1. X.690 §8.1.2.5.
    #[must_use]
    pub const fn is_constructed(&self) -> bool {
        self.tag.constructed
    }

    /// True when the length is the indefinite form. X.690 §8.1.3.6.
    #[must_use]
    pub const fn is_indefinite(&self) -> bool {
        matches!(self.length, Length::Indefinite)
    }

    /// The exact identifier and length octets. X.690 §8.1.1.1.
    #[must_use]
    pub const fn header(&self) -> &'a [u8] {
        self.header
    }

    /// The exact contents octets, without any end-of-contents octets. X.690 §8.1.4.
    #[must_use]
    pub const fn content(&self) -> &'a [u8] {
        self.content
    }

    /// The whole encoding as it stood in the input: header, contents and, for the indefinite form, the two
    /// end-of-contents octets. X.690 §8.1.1.1.
    #[must_use]
    pub const fn encoding(&self) -> &'a [u8] {
        self.encoding
    }

    /// The offset of the first identifier octet in the slice given to [`Tlv::parse`].
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// The nested encodings of a constructed element, in order, without allocating. X.690 §8.1.4, §8.9.2.
    ///
    /// A primitive element has no nested encodings, so its iterator is empty (the typed layer decides
    /// whether the constructed bit is right for the type). After the first error the iterator ends.
    #[must_use]
    pub const fn children(&self) -> Children<'a> {
        let region = if self.tag.constructed {
            self.content
        } else {
            &[]
        };
        Children {
            region,
            pos: 0,
            base: self.offset + self.header.len(),
            level: self.level + 1,
            mode: self.mode,
            limits: self.limits,
            done: false,
        }
    }
}

/// The children of a constructed [`Tlv`]. Each item is one element or the first error, after which it ends.
#[derive(Clone, Debug)]
pub struct Children<'a> {
    region: &'a [u8],
    pos: usize,
    base: usize,
    level: usize,
    mode: Mode,
    limits: Limits,
    done: bool,
}

impl<'a> Iterator for Children<'a> {
    type Item = Result<Tlv<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.pos >= self.region.len() {
            return None;
        }
        let ctx = Ctx {
            mode: self.mode,
            limits: self.limits,
            bounded: true,
        };
        match next_item(
            &self.region[self.pos..],
            ctx,
            false,
            self.level,
            self.base + self.pos,
        ) {
            Ok(Item::Element(t)) => {
                self.pos += t.encoding.len();
                Some(Ok(t))
            }
            Ok(Item::Eoc) => {
                self.done = true;
                Some(Err(Error::new(STRAY_EOC, self.base + self.pos)))
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

/// Why a writer refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// The output buffer is too small; `needed` is the size of the whole encoding.
    BufferTooSmall(BufferTooSmall),
    /// The tag is UNIVERSAL 0, reserved for the end-of-contents octets (X.680 §8.4 Table 1, X.690 §8.1.5).
    /// Use [`write_eoc`] for those.
    ReservedTag,
    /// The indefinite length was asked for a primitive tag. X.690 §8.1.3.2 a).
    IndefiniteOnPrimitive,
}

impl From<BufferTooSmall> for WriteError {
    fn from(e: BufferTooSmall) -> Self {
        Self::BufferTooSmall(e)
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall(e) => e.fmt(f),
            Self::ReservedTag => {
                f.write_str("UNIVERSAL 0 is reserved for the end-of-contents octets")
            }
            Self::IndefiniteOnPrimitive => {
                f.write_str("the indefinite length needs a constructed tag")
            }
        }
    }
}

impl core::error::Error for WriteError {}

/// The number of octets of a definite-length encoding of `content_len` contents octets under `tag`.
/// X.690 §8.1.1.1.
#[must_use]
pub const fn encoded_len(tag: &Tag, content_len: usize) -> usize {
    tag.encoded_len() + Length::Definite(content_len as u64).encoded_len() + content_len
}

/// Write the identifier and length octets (the header) and return the octets written. The length is written
/// in its minimal form. X.690 §8.1.2, §8.1.3, §10.1.
///
/// # Errors
/// [`WriteError::ReservedTag`] for UNIVERSAL 0, [`WriteError::IndefiniteOnPrimitive`] for the indefinite
/// length on a primitive tag, [`WriteError::BufferTooSmall`] when `out` cannot hold the header.
pub fn write_header(tag: &Tag, length: Length, out: &mut [u8]) -> Result<usize, WriteError> {
    if tag.is_universal_zero() {
        return Err(WriteError::ReservedTag);
    }
    if length == Length::Indefinite && !tag.constructed {
        return Err(WriteError::IndefiniteOnPrimitive);
    }
    let needed = tag.encoded_len() + length.encoded_len();
    if out.len() < needed {
        return Err(BufferTooSmall { needed }.into());
    }
    let t = tag.encode(out)?;
    let l = length.encode(&mut out[t..])?;
    Ok(t + l)
}

/// Write a whole definite-length encoding of `content` under `tag` and return the octets written.
/// X.690 §8.1.1.1, §8.1.3.3. On [`WriteError::BufferTooSmall`] nothing is written.
///
/// # Errors
/// As [`write_header`], with `needed` counting the whole encoding.
pub fn write_tlv(tag: &Tag, content: &[u8], out: &mut [u8]) -> Result<usize, WriteError> {
    let needed = encoded_len(tag, content.len());
    if out.len() < needed {
        return Err(BufferTooSmall { needed }.into());
    }
    let h = write_header(tag, Length::Definite(content.len() as u64), out)?;
    out[h..h + content.len()].copy_from_slice(content);
    Ok(h + content.len())
}

/// Write the end-of-contents octets `00 00` that close an indefinite-length encoding. X.690 §8.1.5 ¶2.
///
/// # Errors
/// [`WriteError::BufferTooSmall`].
pub fn write_eoc(out: &mut [u8]) -> Result<usize, WriteError> {
    if out.len() < 2 {
        return Err(BufferTooSmall { needed: 2 }.into());
    }
    out[0] = 0;
    out[1] = 0;
    Ok(2)
}

/// A context-specific, constructed tag, a convenience for callers and tests.
#[must_use]
pub const fn context(number: u64, constructed: bool) -> Tag {
    Tag::new(Class::ContextSpecific, constructed, number)
}
