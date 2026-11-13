//! Restricted character string alphabets and validators. X.690 §8.23, X.680 §41 to §43.
//!
//! Every function here takes the CONTENTS octets of one string value. The identifier and length octets are the
//! framing module's concern, and so is deciding whether the string arrived in the constructed form (X.690
//! §8.23.6): a constructed string is validated by feeding its segment contents, in order, to
//! [`validate_segments`] or to a [`Validator`], never one segment at a time, because a segment boundary may fall
//! inside a character (X.690 §8.7.3.1, no significance on boundaries).
//!
//! The first violation is returned as [`ErrorClass::BadContents`] with the offset of the first bad octet, counted
//! from the start of the contents (for segments: from the start of the first segment, over all segments). For a
//! character cut short by the end of the value the offset is that of the character's first octet.
//!
//! What a receiver does with an octet outside an alphabet is not stated by X.690 or X.680 (the notes call it
//! POLICY, S10): this crate rejects it, because such an octet does not denote a value of the type.
//!
//! The mode does not change any alphabet. In DER the only string-specific rule is the form: see [`check_form`].

use crate::{Error, ErrorClass, Mode};

/// A character string type of X.680 §41. The universal tag number is given by [`StringType::tag_number`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StringType {
    /// `NumericString`: 11 characters. X.680 §41.2, Table 9, §43.5.
    Numeric,
    /// `PrintableString`: 74 characters. X.680 §41.4, Table 10, §43.6.
    Printable,
    /// `VisibleString` (ISO646String): cells 2/0 to 7/14. X.680 Table 8, §43.7.
    Visible,
    /// `IA5String`: 128 characters. X.680 Table 8, §43.8.
    Ia5,
    /// `UTF8String`. X.690 §8.23.10, RFC 3629 (POLICY for what RFC 3629 decides, `[unread]` ISO/IEC 10646 Annex D).
    Utf8,
    /// `BMPString`: two octets per character. X.690 §8.23.8, X.680 §41.15.
    Bmp,
    /// `UniversalString`: four octets per character. X.690 §8.23.7, X.680 §41.6. `[unread]` ISO/IEC 10646.
    Universal,
    /// `TeletexString` (`T61String`). No alphabet check: `[unread]` ISO register entries. X.680 Table 8.
    Teletex,
    /// `VideotexString`. No alphabet check: `[unread]` ISO register entries. X.680 Table 8.
    Videotex,
    /// `GraphicString`. No alphabet check: `[unread]` ISO register entries. X.680 Table 8.
    Graphic,
    /// `GeneralString`. No alphabet check: `[unread]` ISO register entries. X.680 Table 8.
    General,
}

impl StringType {
    /// The universal tag number. X.680 Table 1.
    #[must_use]
    pub const fn tag_number(self) -> u8 {
        match self {
            Self::Utf8 => 12,
            Self::Numeric => 18,
            Self::Printable => 19,
            Self::Teletex => 20,
            Self::Videotex => 21,
            Self::Ia5 => 22,
            Self::Graphic => 25,
            Self::Visible => 26,
            Self::General => 27,
            Self::Universal => 28,
            Self::Bmp => 30,
        }
    }
}

/// Whether a string in the constructed form (X.690 §8.23.6) is allowed in `mode`.
///
/// BER accepts both forms (X.690 §8.23.6, §7.3). DER refuses the constructed form with
/// [`ErrorClass::ConstructedString`] (X.690 §10.2 ¶1: "for bitstring, octetstring and restricted character string
/// types, the constructed form of encoding shall not be used"). The offset is 0 (the identifier octet).
///
/// Whether §10.2 also covers `UTCTime`, `GeneralizedTime` and `ObjectDescriptor` is NOT settled by the text: those
/// are not in X.680 §41's list of restricted character string types, but X.690 §8.25 encodes the two time types
/// "as if" they were `IMPLICIT VisibleString` (so by §8.23.3 as octet strings), and `ObjectDescriptor` is defined
/// as an IMPLICIT `GraphicString`. The same function is used by [`crate::time::check_form`] under the POLICY that
/// they are covered (a DER value has one encoding, X.690 §7.4).
///
/// # Errors
/// [`ErrorClass::ConstructedString`] when `constructed` and `mode` is DER.
pub const fn check_form(mode: Mode, constructed: bool) -> Result<(), Error> {
    if constructed && mode.is_der() {
        Err(Error::new(ErrorClass::ConstructedString, 0))
    } else {
        Ok(())
    }
}

const fn bad(offset: usize) -> Error {
    Error::new(ErrorClass::BadContents, offset)
}

/// True if `b` is a `NumericString` character: SPACE and the digits. X.680 §41.2 Table 9, §43.5.
#[must_use]
pub const fn is_numeric_octet(b: u8) -> bool {
    matches!(b, b' ' | b'0'..=b'9')
}

/// True if `b` is a `PrintableString` character. X.680 §41.4 Table 10, §43.6.
///
/// The letters, the digits, SPACE, APOSTROPHE (0x27), `(` `)` `+` `,` `-` `.` `/` `:` `=` `?`. Not `@`, `*`, `&`,
/// `!`, `"`, `#`, `$`, `%`, `;`, `<`, `>`, `_`.
#[must_use]
pub const fn is_printable_octet(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b' '
            | b'\''
            | b'('
            | b')'
            | b'+'
            | b','
            | b'-'
            | b'.'
            | b'/'
            | b':'
            | b'='
            | b'?'
    )
}

/// True if `b` is a `VisibleString` character: cells 2/0 to 7/14 (0x20 to 0x7E). X.680 Table 8, §43.7.
#[must_use]
pub const fn is_visible_octet(b: u8) -> bool {
    matches!(b, 0x20..=0x7E)
}

/// True if `b` is an `IA5String` character: 0x00 to 0x7F, including DELETE. X.680 Table 8, §43.8.
#[must_use]
pub const fn is_ia5_octet(b: u8) -> bool {
    b <= 0x7F
}

/// The cell range of a surrogate, which is not a `BMPString` character. X.680 §41.15.
const fn is_surrogate(cp: u32) -> bool {
    matches!(cp, 0xD800..=0xDFFF)
}

/// An incremental validator over the logical concatenation of the segments of one string.
///
/// Feed every segment contents in order with [`Validator::feed`], then call [`Validator::finish`]. No
/// allocation: it keeps the state of the character in progress, so a character split across two segments is
/// validated as one. X.690 §8.7.3.1 ¶2 (segment boundaries carry no significance), §8.23.3.
#[derive(Clone, Copy, Debug)]
pub struct Validator {
    ty: StringType,
    /// Octets consumed so far, over all segments.
    offset: usize,
    /// Octets still needed to complete the character in progress.
    need: u8,
    /// Offset of the first octet of the character in progress.
    start: usize,
    /// Bits of the character in progress (BMP, UCS-4).
    acc: u32,
    /// UTF-8: the range the next octet must fall in.
    lo: u8,
    hi: u8,
}

impl Validator {
    /// A validator for strings of type `ty`.
    #[must_use]
    pub const fn new(ty: StringType) -> Self {
        Self {
            ty,
            offset: 0,
            need: 0,
            start: 0,
            acc: 0,
            lo: 0x80,
            hi: 0xBF,
        }
    }

    /// Check the next run of octets.
    ///
    /// # Errors
    /// [`ErrorClass::BadContents`] at the first bad octet.
    pub fn feed(&mut self, octets: &[u8]) -> Result<(), Error> {
        for &b in octets {
            self.step(b)?;
            self.offset += 1;
        }
        Ok(())
    }

    /// End of the value: fails if a character is incomplete (truncated UTF-8, odd `BMPString` length, `UniversalString`
    /// length not a multiple of four). The offset is that of the incomplete character's first octet.
    ///
    /// # Errors
    /// [`ErrorClass::BadContents`].
    pub const fn finish(self) -> Result<(), Error> {
        if self.need != 0 {
            Err(bad(self.start))
        } else {
            Ok(())
        }
    }

    fn step(&mut self, b: u8) -> Result<(), Error> {
        let ok = match self.ty {
            StringType::Numeric => is_numeric_octet(b),
            StringType::Printable => is_printable_octet(b),
            StringType::Visible => is_visible_octet(b),
            StringType::Ia5 => is_ia5_octet(b),
            // `[unread]`: the ISO register entries that define these four are not available, so any octet passes.
            StringType::Teletex
            | StringType::Videotex
            | StringType::Graphic
            | StringType::General => true,
            StringType::Utf8 => return self.step_utf8(b),
            StringType::Bmp => return self.step_wide(b, 2),
            StringType::Universal => return self.step_wide(b, 4),
        };
        if ok { Ok(()) } else { Err(bad(self.offset)) }
    }

    /// RFC 3629 §4 grammar (UTF8-1 to UTF8-4). The octet that first breaks it is the offset reported.
    fn step_utf8(&mut self, b: u8) -> Result<(), Error> {
        if self.need == 0 {
            let (need, lo, hi) = match b {
                0x00..=0x7F => return Ok(()),
                0xC2..=0xDF => (1, 0x80, 0xBF),
                0xE0 => (2, 0xA0, 0xBF),
                0xE1..=0xEC | 0xEE..=0xEF => (2, 0x80, 0xBF),
                0xED => (2, 0x80, 0x9F),
                0xF0 => (3, 0x90, 0xBF),
                0xF1..=0xF3 => (3, 0x80, 0xBF),
                0xF4 => (3, 0x80, 0x8F),
                // 0x80..=0xBF lone continuation; 0xC0, 0xC1 overlong lead (X.690 §8.23.10 ¶2); 0xF5.. above U+10FFFF.
                _ => return Err(bad(self.offset)),
            };
            self.need = need;
            self.lo = lo;
            self.hi = hi;
            self.start = self.offset;
            Ok(())
        } else if b < self.lo || b > self.hi {
            Err(bad(self.offset))
        } else {
            self.lo = 0x80;
            self.hi = 0xBF;
            self.need -= 1;
            Ok(())
        }
    }

    /// `BMPString` (`width` 2) and `UniversalString` (`width` 4): big-endian code points.
    fn step_wide(&mut self, b: u8, width: u8) -> Result<(), Error> {
        if self.need == 0 {
            self.need = width;
            self.start = self.offset;
            self.acc = 0;
        }
        self.acc = (self.acc << 8) | u32::from(b);
        self.need -= 1;
        if self.need != 0 {
            return Ok(());
        }
        let cp = self.acc;
        // X.690 §8.23.7 ¶2, §8.23.8 ¶2: signatures shall not be used. A signature is U+FEFF at the start of the
        // value (ISO/IEC 10646, `[unread]`).
        let signature = self.start == 0 && cp == 0xFEFF;
        // POLICY (`[unread]` ISO/IEC 10646): a surrogate cell is no character of either type (X.680 §41.15 for
        // `BMPString`), `UniversalString` stops at U+10FFFF, `BMPString` also refuses U+FFFE and U+FFFF. The last is
        // the notes' inference from "first 64K-2 cells" (X.680 §41.15), NOT a statement of the standard.
        let out_of_set = is_surrogate(cp)
            || match self.ty {
                StringType::Bmp => cp >= 0xFFFE,
                _ => cp > 0x0010_FFFF,
            };
        if signature || out_of_set {
            Err(bad(self.start))
        } else {
            Ok(())
        }
    }
}

/// Validate a string whose contents arrive as segments (the constructed form, X.690 §8.23.6).
///
/// The caller supplies the contents of each segment, in order, with any nested constructed segments already
/// flattened (X.690 §8.7.3.2). The whole value is validated as one, so a character may be split across
/// segments. An empty iterator is the empty string.
///
/// # Errors
/// [`ErrorClass::BadContents`] at the first bad octet, counted over all segments.
pub fn validate_segments<'a, I>(ty: StringType, segments: I) -> Result<(), Error>
where
    I: IntoIterator<Item = &'a [u8]>,
{
    let mut v = Validator::new(ty);
    for s in segments {
        v.feed(s)?;
    }
    v.finish()
}

/// Validate the contents of a string of type `ty`. X.690 §8.23, X.680 §41.
///
/// # Errors
/// [`ErrorClass::BadContents`] at the first bad octet.
pub fn validate(ty: StringType, content: &[u8]) -> Result<(), Error> {
    validate_segments(ty, core::iter::once(content))
}

/// Validate `NumericString` contents. X.680 §41.2, §43.5.
///
/// # Errors
/// [`ErrorClass::BadContents`] at the first octet other than SPACE or a digit.
pub fn validate_numeric(content: &[u8]) -> Result<(), Error> {
    validate(StringType::Numeric, content)
}

/// Validate `PrintableString` contents. X.680 §41.4, §43.6.
///
/// # Errors
/// [`ErrorClass::BadContents`] at the first octet outside the 74 characters.
pub fn validate_printable(content: &[u8]) -> Result<(), Error> {
    validate(StringType::Printable, content)
}

/// Validate `VisibleString` contents. X.680 §43.7.
///
/// # Errors
/// [`ErrorClass::BadContents`] at the first octet outside 0x20 to 0x7E.
pub fn validate_visible(content: &[u8]) -> Result<(), Error> {
    validate(StringType::Visible, content)
}

/// Validate `IA5String` contents. X.680 §43.8.
///
/// # Errors
/// [`ErrorClass::BadContents`] at the first octet above 0x7F.
pub fn validate_ia5(content: &[u8]) -> Result<(), Error> {
    validate(StringType::Ia5, content)
}

/// Validate `UTF8String` contents. X.690 §8.23.10 (overlong forms refused, ¶2), RFC 3629 §4 for the rest.
///
/// Surrogates, values above U+10FFFF and truncated sequences are refused under RFC 3629; X.680 and X.690
/// do not decide them (ISO/IEC 10646 Annex D does, `[unread]`), so those rules are POLICY.
///
/// # Errors
/// [`ErrorClass::BadContents`].
pub fn validate_utf8(content: &[u8]) -> Result<(), Error> {
    validate(StringType::Utf8, content)
}

/// Validate `BMPString` contents: two octets per character. X.690 §8.23.8, X.680 §41.15.
///
/// Odd length, surrogate cells and a leading signature U+FEFF are refused. U+FFFE and U+FFFF are refused as
/// POLICY (not a statement of the standard).
///
/// # Errors
/// [`ErrorClass::BadContents`].
pub fn validate_bmp(content: &[u8]) -> Result<(), Error> {
    validate(StringType::Bmp, content)
}

/// Validate `UniversalString` contents: four octets per character. X.690 §8.23.7.
///
/// Length not a multiple of four, a leading signature, surrogates and values above U+10FFFF are refused. The
/// range is decided by ISO/IEC 10646 (`[unread]`); this is the POLICY chosen.
///
/// # Errors
/// [`ErrorClass::BadContents`].
pub fn validate_universal(content: &[u8]) -> Result<(), Error> {
    validate(StringType::Universal, content)
}
