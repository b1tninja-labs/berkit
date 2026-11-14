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
//!
//! # Stack use
//!
//! The reader does not recurse: nesting is followed with an explicit stack of frames of 24 octets each (12 on a
//! 32-bit target). The stack has 8, 64 or 512 frames, the smallest that holds `max_depth` (never more than
//! [`Limits::HARD_MAX_DEPTH`]): 192, 1,536 or 12,288 octets (96, 768 or 6,144 on 32-bit), so the default limit of
//! 64 levels costs 1,536 octets and no limit at all costs 12,288. It is a local array of the call that needs it:
//! [`Tlv::validate`] on a definite element, and [`Tlv::parse`] or an iteration of [`Children`] when the element
//! read has the indefinite length. That is the whole cost of depth: it does not depend on the input, in any
//! build, and `Tlv`, [`Children`] and the other public types are unchanged. The depth bound is therefore a bound
//! on the work and on the offsets reported, no longer on the stack. The one recursion left in the crate is over a
//! caller-built schema (`typed`), which the caller bounds.

use crate::length::{self, Length, LongFormError};
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
    /// is `limit_exceeded`. The reader reads at most this deep, and never deeper than
    /// [`Limits::HARD_MAX_DEPTH`] whatever is requested here. Default 64.
    pub max_depth: usize,
    /// The largest definite contents length accepted, in octets. A longer one that fits the input is
    /// `limit_exceeded`. A length larger than the input is `truncated` or `length_overrun` first, because the
    /// octets are not there. Default 256 MiB.
    pub max_content_length: u64,
}

impl Limits {
    /// The deepest nesting the reader will ever read, whatever `max_depth` says. It is also the size, in frames,
    /// of the largest explicit stack the reader uses (see the module documentation), so the stack the reader uses
    /// never depends on the input.
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

/// Read the header of one element, or one end-of-contents, at the start of `input`, and check what the header
/// alone decides: identifier, length form, the depth bound and, for a definite length, the extent. `base` is the
/// offset of `input[0]` in the top-level input; `level` is the nesting level of the element (the top level is 1).
///
/// This function does not recurse. For the indefinite form it returns the element with PROVISIONAL contents and
/// encoding (everything after the header, and the whole of `input`): the end-of-contents octets are found by
/// [`walk`], which fixes both up.
fn read_element<'a>(
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
        Length::Indefinite => Ok(Item::Element(make(&input[header_len..], input))),
    }
}

/// Read one element, or one end-of-contents, at the start of `input`. `base` is the offset of `input[0]` in
/// the top-level input; `level` is the nesting level of the element (the top level is 1).
///
/// A definite element is checked as far as its header and extent. An indefinite one is read in full, validating
/// everything inside in stream order, by [`walk`].
fn next_item(
    input: &[u8],
    ctx: Ctx,
    indef_open: bool,
    level: usize,
    base: usize,
) -> Result<Item<'_>, Error> {
    let t = match read_element(input, ctx, indef_open, level, base)? {
        Item::Element(t) if t.length == Length::Indefinite => t,
        done => return Ok(done),
    };
    let header_len = t.header.len();
    let root = Frame {
        pos: header_len,
        end: input.len(),
        indefinite: true,
        bounded: ctx.bounded,
    };
    let eoc = walk(input, base, level, ctx.mode, ctx.limits, root)?;
    Ok(Item::Element(Tlv {
        content: &input[header_len..eoc],
        encoding: &input[..eoc + 2],
        ..t
    }))
}

/// The sizes, in frames, of the explicit stack of [`walk`]: the smallest of the three that holds
/// `Limits::depth()` frames. The last is [`Limits::HARD_MAX_DEPTH`], so no input and no limit needs more.
///
/// A frame stands for one open constructed element, and an element deeper than `Limits::depth()` is refused
/// by [`read_element`] before it can be pushed, so at most `depth() - root_level + 1` frames are open: the root
/// frame, at level `root_level >= 1`, and one for each deeper level up to `depth()`.
const FRAMES_SMALL: usize = 8;
const FRAMES_MEDIUM: usize = 64;
const FRAMES_LARGE: usize = Limits::HARD_MAX_DEPTH;

// The default limits fit the medium stack, and nothing is deeper than the large one.
const _: () = assert!(Limits::DEFAULT.max_depth <= FRAMES_MEDIUM);
const _: () = assert!(FRAMES_SMALL < FRAMES_MEDIUM && FRAMES_MEDIUM < FRAMES_LARGE);

/// One open constructed element of [`walk`]: 24 octets on a 64-bit target (two `usize` and two `bool`, padded),
/// 12 on a 32-bit one. The stacks of 8, 64 and 512 frames are 192, 1,536 and 12,288 octets (96, 768 and 6,144
/// on 32-bit), whatever the input.
///
/// The level is not stored: it is the root's level plus the frame's index in the stack.
#[derive(Clone, Copy)]
struct Frame {
    /// The offset in the walked slice of the next octet to read.
    pos: usize,
    /// The end of the region that holds the children: the end of the contents of a definite element, and for an
    /// indefinite one the end of the region it was read from (its end-of-contents octets lie before it).
    end: usize,
    /// True for the indefinite form: the region ends at its end-of-contents octets, and running out of input
    /// first is `missing_eoc`.
    indefinite: bool,
    /// The `bounded` of [`Ctx`] for the items read in this region (see [`Ctx::bounded`]): always true inside a
    /// definite element, inherited by an indefinite one from the region it lies in.
    bounded: bool,
}

impl Frame {
    const EMPTY: Self = Self {
        pos: 0,
        end: 0,
        indefinite: false,
        bounded: false,
    };
}

/// Walk the tree below one open element in stream order, depth first, checking every element, without
/// recursion: the open elements are the frames of an explicit stack of fixed size (8, 64 or 512 frames, by
/// `limits`), so the stack this takes does not depend on the nesting of the input.
///
/// `data` is the slice the walk reads, `origin` the offset of `data[0]` in the top-level input, and `root_level`
/// the nesting level of the element `root` stands for. A definite child met on the way is walked at once, as
/// [`Tlv::validate`] does, so a fault inside it comes before any later fault. Returns, for an indefinite root,
/// the offset in `data` of its end-of-contents octets, and for a definite one the end of its region.
fn walk(
    data: &[u8],
    origin: usize,
    root_level: usize,
    mode: Mode,
    limits: Limits,
    root: Frame,
) -> Result<usize, Error> {
    let depth = limits.depth();
    if depth <= FRAMES_SMALL {
        walk_in::<FRAMES_SMALL>(data, origin, root_level, mode, limits, root)
    } else if depth <= FRAMES_MEDIUM {
        walk_in::<FRAMES_MEDIUM>(data, origin, root_level, mode, limits, root)
    } else {
        walk_in::<FRAMES_LARGE>(data, origin, root_level, mode, limits, root)
    }
}

/// [`walk`] with a stack of `N` frames, `N >= limits.depth()`.
///
/// Never inlined: inlined into [`walk`], the three arrays would share one stack frame and add up.
#[inline(never)]
fn walk_in<const N: usize>(
    data: &[u8],
    origin: usize,
    root_level: usize,
    mode: Mode,
    limits: Limits,
    root: Frame,
) -> Result<usize, Error> {
    let mut stack = [Frame::EMPTY; N];
    stack[0] = root;
    let mut top = 0;
    loop {
        let f = stack[top];
        let at = origin + f.pos;
        if f.pos >= f.end {
            // A definite region ends cleanly here; an indefinite one ran out before its end-of-contents octets.
            if f.indefinite {
                return Err(Error::new(ErrorClass::MissingEoc, at));
            }
            if top == 0 {
                return Ok(f.pos);
            }
            top -= 1;
            continue;
        }
        let ctx = Ctx {
            mode,
            limits,
            bounded: f.bounded,
        };
        match read_element(
            &data[f.pos..f.end],
            ctx,
            f.indefinite,
            root_level + top + 1,
            at,
        )? {
            Item::Eoc if f.indefinite => {
                if top == 0 {
                    return Ok(f.pos);
                }
                // The element is closed: its parent resumes after the two end-of-contents octets.
                top -= 1;
                stack[top].pos = f.pos + 2;
            }
            // Unreachable: only an open indefinite element lets `read_element` return the octets as an Eoc.
            Item::Eoc => return Err(Error::new(STRAY_EOC, at)),
            Item::Element(t) => {
                let child = if t.length == Length::Indefinite {
                    // Its extent is known only at its end-of-contents octets, where the parent resumes.
                    Frame {
                        pos: f.pos + t.header.len(),
                        end: f.end,
                        indefinite: true,
                        bounded: f.bounded,
                    }
                } else {
                    stack[top].pos = f.pos + t.encoding.len();
                    if !t.tag.constructed || t.content.is_empty() {
                        continue;
                    }
                    Frame {
                        pos: f.pos + t.header.len(),
                        end: f.pos + t.encoding.len(),
                        indefinite: false,
                        bounded: true,
                    }
                };
                // Cannot fail: `N >= limits.depth()`, see `FRAMES_SMALL`.
                let Some(slot) = stack.get_mut(top + 1) else {
                    return Err(Error::new(ErrorClass::LimitExceeded, at));
                };
                *slot = child;
                top += 1;
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

    /// Check every descendant, depth first, in stream order. The walk is iterative: it uses a stack of fixed size
    /// (see the module documentation), not one that grows with the nesting of the input.
    ///
    /// # Errors
    /// The first fault in stream order.
    pub fn validate(&self) -> Result<(), Error> {
        // An indefinite-length element was walked in full when it was read.
        if self.length == Length::Indefinite {
            return Ok(());
        }
        // A primitive element has no children, whatever its contents (as in `children`).
        let region = if self.tag.constructed {
            self.content
        } else {
            &[]
        };
        if region.is_empty() {
            return Ok(());
        }
        let root = Frame {
            pos: 0,
            end: region.len(),
            indefinite: false,
            bounded: true,
        };
        walk(
            region,
            self.offset + self.header.len(),
            self.level,
            self.mode,
            self.limits,
            root,
        )
        .map(|_| ())
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
    /// The long-form length could not be written. See [`length::encode_long`].
    LongForm(LongFormError),
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
            Self::LongForm(e) => e.fmt(f),
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

/// Write the identifier octets and a definite length in the long form with exactly `octets` length octets (BER
/// only: DER takes the minimal form, which [`write_header`] writes). Returns the octets written.
/// X.690 §8.1.2, §8.1.3.5 and its NOTE 2, §10.1.
///
/// # Errors
/// [`WriteError::ReservedTag`] for UNIVERSAL 0, [`WriteError::LongForm`] as [`length::encode_long`] (the octet
/// count, a value that does not fit, a buffer that is too small).
pub fn write_header_long(
    tag: &Tag,
    value: u64,
    octets: usize,
    out: &mut [u8],
) -> Result<usize, WriteError> {
    if tag.is_universal_zero() {
        return Err(WriteError::ReservedTag);
    }
    let t = tag.encode(out)?;
    let l = length::encode_long(value, octets, &mut out[t..]).map_err(WriteError::LongForm)?;
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

// The recursive reader this one replaced, kept as the oracle of a differential test (neither is compiled outside
// tests).
#[cfg(test)]
mod differential;
#[cfg(test)]
mod oracle;
