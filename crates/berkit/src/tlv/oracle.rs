//! The recursive reader this crate had before the iterative one, kept ONLY as the oracle of the differential
//! test (`differential.rs`). It is the code of commit a6560db (`next_item`, `Tlv::parse`, `Tlv::parse_exact`,
//! `Tlv::validate`, `Tlv::children` and the `Children` iterator), copied unchanged except that the methods are
//! free functions over a [`Tlv`], and that `validate` and `Children` call this module's own `next_item`.
//! It recurses once per nesting level, so a test that runs it needs a large stack.

use super::*;
use crate::length;

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

/// `Tlv::parse` of commit a6560db.
pub fn parse<'a>(
    input: &'a [u8],
    mode: Mode,
    limits: &Limits,
) -> Result<(Tlv<'a>, &'a [u8]), Error> {
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
        Item::Eoc => Err(Error::new(STRAY_EOC, 0)),
    }
}

/// `Tlv::parse_exact` of commit a6560db.
pub fn parse_exact<'a>(input: &'a [u8], mode: Mode, limits: &Limits) -> Result<Tlv<'a>, Error> {
    let (tlv, rest) = parse(input, mode, limits)?;
    validate(&tlv)?;
    if rest.is_empty() {
        Ok(tlv)
    } else {
        Err(Error::new(ErrorClass::TrailingData, tlv.encoding.len()))
    }
}

/// `Tlv::validate` of commit a6560db.
pub fn validate(t: &Tlv<'_>) -> Result<(), Error> {
    if t.length == Length::Indefinite {
        return Ok(());
    }
    for child in children(t) {
        validate(&child?)?;
    }
    Ok(())
}

/// `Tlv::children` of commit a6560db.
pub fn children<'a>(t: &Tlv<'a>) -> Children<'a> {
    let region = if t.tag.constructed { t.content } else { &[] };
    Children {
        region,
        pos: 0,
        base: t.offset + t.header.len(),
        level: t.level + 1,
        mode: t.mode,
        limits: t.limits,
        done: false,
    }
}

/// The `Children` iterator of commit a6560db.
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
