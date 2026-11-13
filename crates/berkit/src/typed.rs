//! Schema-driven validation: where the TLV layer and the content layers meet.
//!
//! The crate has a content function per type and a TLV reader that never looks at the type
//! (`docs/design/structured.md` section 5). This module does the type-dependent work in between, over a
//! caller-built [`Schema`] and without allocating:
//!
//! 1. the IDENTIFIER decides the form before any length fault (`wrong_form`; in DER `constructed_string`),
//!    X.690 §8.1.2.5, §8.2.1, §8.3.1, §8.9.1, §10.2;
//! 2. the reader parses and validates the structure;
//! 3. the content function of the matching module runs with the mode and flags the type needs (named-bit
//!    list, `UTCTime` century pivot, string alphabet);
//! 4. constructed strings are read through their segments (X.690 §8.6.4, §8.7.3, §8.23.6), DER `SET`
//!    component order (§10.3) and `SET OF` order (§11.6) are checked, and a `DEFAULT` component that is
//!    present is refused in DER (§11.5).
//!
//! A schema describes what the caller's ASN.1 module says; it is built from the module, not from the
//! encoding, so a tag that does not match is a [`Mismatch`], not an encoding error: X.690 has no rule for it.
//! No value is returned: a caller that wants one decodes the content with the module for the type, after
//! this module has validated it.
//!
//! Limits that are POLICY, not standard: a constructed string may nest [`MAX_SEGMENT_DEPTH`] levels, a
//! constructed time string may hold [`MAX_TIME_OCTETS`] octets, and a `SET` may name 64 components.

use crate::strings::{self, StringType};
use crate::tag::{Class, Tag};
use crate::tlv::{Children, Limits, Tlv};
use crate::{
    Error, ErrorClass, Mode, bitstring, boolean, integer, null, octetstring, oid, real, time,
};
use core::fmt;

/// The deepest nesting of a constructed string's segments this module follows without allocating.
pub const MAX_SEGMENT_DEPTH: usize = 16;
/// The longest constructed `UTCTime` or `GeneralizedTime` this module joins into one buffer.
pub const MAX_TIME_OCTETS: usize = 64;
/// The most components a `SET` schema may name (the presence record is a 64-bit mask).
pub const MAX_SET_FIELDS: usize = 64;

/// A universal type with primitive contents rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prim {
    /// BOOLEAN, X.690 §8.2.
    Boolean,
    /// INTEGER, §8.3.
    Integer,
    /// ENUMERATED, §8.4 (the INTEGER contents rule).
    Enumerated,
    /// BIT STRING; `named` is true for a type defined with a `NamedBitList` (X.680 §22.7).
    BitString {
        /// Whether trailing zero bits must be removed in DER.
        named: bool,
    },
    /// OCTET STRING, §8.7.
    OctetString,
    /// NULL, §8.8.
    Null,
    /// OBJECT IDENTIFIER, §8.19.
    Oid,
    /// RELATIVE-OID, §8.20.
    RelOid,
    /// REAL, §8.5.
    Real,
    /// `UTCTime`, §8.26 and X.680 §47.
    Utc,
    /// `GeneralizedTime`, §8.26 and X.680 §46.
    Generalized,
    /// DATE, §8.26.
    Date,
    /// TIME-OF-DAY, §8.26.
    TimeOfDay,
    /// DATE-TIME, §8.26.
    DateTime,
    /// DURATION, §8.26.
    Duration,
    /// TIME, §8.26.
    Time,
    /// OID-IRI, §8.27.
    OidIri,
    /// RELATIVE-OID-IRI, §8.28.
    RelOidIri,
    /// A restricted character string.
    Str(StringType),
}

impl Prim {
    /// The UNIVERSAL tag number. X.680 §8.6 Table 1.
    #[must_use]
    pub fn number(self) -> u64 {
        match self {
            Prim::Boolean => 1,
            Prim::Integer => 2,
            Prim::BitString { .. } => 3,
            Prim::OctetString => 4,
            Prim::Null => 5,
            Prim::Oid => 6,
            Prim::Real => 9,
            Prim::Enumerated => 10,
            Prim::RelOid => 13,
            Prim::Time => 14,
            Prim::Utc => 23,
            Prim::Generalized => 24,
            Prim::Date => 31,
            Prim::TimeOfDay => 32,
            Prim::DateTime => 33,
            Prim::Duration => 34,
            Prim::OidIri => 35,
            Prim::RelOidIri => 36,
            Prim::Str(t) => u64::from(t.tag_number()),
        }
    }

    /// Types that may be constructed in BER (X.690 §8.6.1, §8.7.1, §8.23.6, §8.25); DER takes the
    /// primitive form only (§10.2).
    #[must_use]
    pub const fn may_be_constructed(self) -> bool {
        matches!(
            self,
            Prim::BitString { .. }
                | Prim::OctetString
                | Prim::Utc
                | Prim::Generalized
                | Prim::Str(_)
        )
    }
}

/// How a component of a `SEQUENCE` or `SET` may be absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    /// The component must be present.
    Required,
    /// `OPTIONAL`.
    Optional,
    /// `DEFAULT n` on an INTEGER: DER leaves the component out when it equals `n` (X.690 §11.5).
    DefaultInt(i128),
}

/// A component of a `SEQUENCE` or `SET`.
#[derive(Clone, Copy, Debug)]
pub struct Field<'s> {
    /// The component's type.
    pub schema: Schema<'s>,
    /// Whether it may be absent.
    pub presence: Presence,
}

/// An ASN.1 type, as far as validation needs it. Built with `const` expressions or at run time.
#[derive(Clone, Copy, Debug)]
pub enum Schema<'s> {
    /// A universal type with primitive contents rules.
    Prim(Prim),
    /// `SEQUENCE`; `None` leaves the components unchecked.
    Sequence(Option<&'s [Field<'s>]>),
    /// `SEQUENCE OF`.
    SequenceOf(&'s Schema<'s>),
    /// `SET`; DER checks the tag order (X.690 §10.3).
    Set(Option<&'s [Field<'s>]>),
    /// `SET OF`; DER checks the encoding order (X.690 §11.6). `None` leaves the elements unchecked.
    SetOf(Option<&'s Schema<'s>>),
    /// A tagged type. `explicit` wraps the inner encoding in a constructed one (X.690 §8.14.2);
    /// implicit replaces the inner identifier (§8.14.3).
    Tagged {
        /// The class of the new tag.
        class: Class,
        /// The number of the new tag.
        number: u64,
        /// Explicit (`true`) or implicit.
        explicit: bool,
        /// The tagged type.
        inner: &'s Schema<'s>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    Primitive,
    Constructed,
    /// Either in BER, primitive only in DER.
    Either,
}

impl Schema<'_> {
    /// The outer identifier (class, number) a value of this type carries.
    #[must_use]
    pub fn outer(&self) -> (Class, u64) {
        match self {
            Schema::Prim(p) => (Class::Universal, p.number()),
            Schema::Sequence(_) | Schema::SequenceOf(_) => (Class::Universal, 16),
            Schema::Set(_) | Schema::SetOf(_) => (Class::Universal, 17),
            Schema::Tagged { class, number, .. } => (*class, *number),
        }
    }

    fn form(&self) -> Form {
        match self {
            Schema::Prim(p) if p.may_be_constructed() => Form::Either,
            Schema::Prim(_) => Form::Primitive,
            Schema::Sequence(_) | Schema::SequenceOf(_) | Schema::Set(_) | Schema::SetOf(_) => {
                Form::Constructed
            }
            Schema::Tagged { explicit: true, .. } => Form::Constructed,
            Schema::Tagged { inner, .. } => inner.form(),
        }
    }
}

/// The encoding does not fit the schema in a way X.690 has no error class for: the schema is the caller's
/// ASN.1 module, and the standard does not say what a receiver reports when the module and the octets
/// disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mismatch {
    /// The identifier is not the one the schema wants.
    Tag {
        /// Class the schema wants.
        want_class: Class,
        /// Number the schema wants.
        want_number: u64,
        /// Class found.
        found_class: Class,
        /// Number found.
        found_number: u64,
    },
    /// A component the schema does not name at this position.
    UnexpectedComponent {
        /// Class of the component.
        class: Class,
        /// Number of the component.
        number: u64,
    },
    /// A `Required` component is absent.
    MissingRequired,
    /// An explicit tag holds other than exactly one encoding.
    ExplicitArity,
    /// The schema or the encoding is beyond a limit of this module, which is policy.
    Capacity,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mismatch::Tag {
                want_class,
                want_number,
                found_class,
                found_number,
            } => write!(
                f,
                "tag mismatch: schema wants {want_class:?} {want_number}, input has {found_class:?} {found_number}"
            ),
            Mismatch::UnexpectedComponent { class, number } => {
                write!(f, "component {class:?} {number} not in the schema here")
            }
            Mismatch::MissingRequired => f.write_str("a required component is missing"),
            Mismatch::ExplicitArity => f.write_str("an explicit tag holds exactly one encoding"),
            Mismatch::Capacity => f.write_str("beyond a limit of the typed layer"),
        }
    }
}

/// Why validation did not succeed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypedError {
    /// The encoding is refused with a corpus error class.
    Class(Error),
    /// The encoding does not fit the schema.
    Mismatch(Mismatch),
}

impl From<Error> for TypedError {
    fn from(e: Error) -> Self {
        TypedError::Class(e)
    }
}

impl From<Mismatch> for TypedError {
    fn from(m: Mismatch) -> Self {
        TypedError::Mismatch(m)
    }
}

impl fmt::Display for TypedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypedError::Class(e) => e.fmt(f),
            TypedError::Mismatch(m) => m.fmt(f),
        }
    }
}

impl core::error::Error for TypedError {}

fn fail<T>(class: ErrorClass, offset: usize) -> Result<T, TypedError> {
    Err(TypedError::Class(Error::new(class, offset)))
}

/// What a validation call needs besides the schema.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    /// BER or DER.
    pub mode: Mode,
    /// The century pivot for `UTCTime`, left to the caller (X.680 §47.3).
    pub pivot: u8,
}

impl Context {
    /// `mode` with the RFC 5280 pivot.
    #[must_use]
    pub const fn new(mode: Mode) -> Self {
        Self {
            mode,
            pivot: time::RFC5280_PIVOT,
        }
    }
}

/// Read `input` as exactly one encoding of `schema` and validate it.
///
/// Stream order: a fault in the identifier (form) comes before the length octets, which come before the
/// contents, and trailing data comes last.
///
/// # Errors
/// An [`Error`] class from the reader or a content function, or a [`Mismatch`].
pub fn validate(
    input: &[u8],
    schema: &Schema<'_>,
    cx: Context,
    limits: &Limits,
) -> Result<(), TypedError> {
    // The identifier is judged before the length. A fault in the identifier itself is left to the reader.
    if let Ok((tag, _)) = Tag::decode(input) {
        if !tag.is_universal_zero() {
            check_identifier(&tag, schema, cx.mode)?;
        }
    }
    let (t, rest) = Tlv::parse(input, cx.mode, limits)?;
    t.validate()?;
    check_content(&t, schema, cx)?;
    if !rest.is_empty() {
        return fail(ErrorClass::TrailingData, t.encoding().len());
    }
    Ok(())
}

/// Judge an identifier against the schema: the tag, then the primitive/constructed bit.
///
/// # Errors
/// [`Mismatch::Tag`]; `wrong_form`; in DER `constructed_string` for a constructed string type.
pub fn check_identifier(tag: &Tag, schema: &Schema<'_>, mode: Mode) -> Result<(), TypedError> {
    let (class, number) = schema.outer();
    if tag.class != class || tag.number != number {
        return Err(Mismatch::Tag {
            want_class: class,
            want_number: number,
            found_class: tag.class,
            found_number: tag.number,
        }
        .into());
    }
    match (schema.form(), tag.constructed) {
        (Form::Primitive, true) | (Form::Constructed, false) => fail(ErrorClass::WrongForm, 0),
        (Form::Either, true) if mode.is_der() => fail(ErrorClass::ConstructedString, 0),
        _ => Ok(()),
    }
}

/// An element of a schema: identifier first, then the contents.
fn check(t: &Tlv<'_>, schema: &Schema<'_>, cx: Context) -> Result<(), TypedError> {
    check_identifier(&t.tag(), schema, cx.mode)?;
    check_content(t, schema, cx)
}

/// Validate the contents of `t` against `schema`, the identifier having been judged.
///
/// # Errors
/// As [`validate`].
pub fn check_content(t: &Tlv<'_>, schema: &Schema<'_>, cx: Context) -> Result<(), TypedError> {
    match schema {
        Schema::Prim(p) => check_prim(t, *p, cx),
        Schema::Tagged {
            explicit: false,
            inner,
            ..
        } => check_content(t, inner, cx),
        Schema::Tagged {
            explicit: true,
            inner,
            ..
        } => {
            let mut kids = t.children();
            let (Some(only), None) = (kids.next(), kids.next()) else {
                return Err(Mismatch::ExplicitArity.into());
            };
            check(&only?, inner, cx)
        }
        Schema::Sequence(None) => {
            drain(t)?;
            Ok(())
        }
        Schema::Sequence(Some(fields)) => check_sequence(t, fields, cx),
        Schema::SequenceOf(inner) => {
            for c in t.children() {
                check(&c?, inner, cx)?;
            }
            Ok(())
        }
        Schema::Set(fields) => check_set(t, *fields, cx),
        Schema::SetOf(inner) => {
            let mut prev: Option<Tlv<'_>> = None;
            for c in t.children() {
                let c = c?;
                if cx.mode.is_der() {
                    if let Some(p) = &prev {
                        if setof_greater(p.encoding(), c.encoding()) {
                            return fail(ErrorClass::SetofOrder, c.offset());
                        }
                    }
                }
                if let Some(inner) = inner {
                    check(&c, inner, cx)?;
                }
                prev = Some(c);
            }
            Ok(())
        }
    }
}

/// Walk the children so that a framing fault inside them is reported, as the reader would.
fn drain(t: &Tlv<'_>) -> Result<(), Error> {
    for c in t.children() {
        c?;
    }
    Ok(())
}

/// X.690 §11.6: shorter encodings are padded with trailing zero octets before comparing.
fn setof_greater(first: &[u8], second: &[u8]) -> bool {
    let len = first.len().max(second.len());
    let padded = |side: &[u8], at: usize| side.get(at).copied().unwrap_or(0);
    (0..len)
        .map(|at| (padded(first, at), padded(second, at)))
        .find(|(left, right)| left != right)
        .is_some_and(|(left, right)| left > right)
}

fn tag_matches(field: &Field<'_>, tag: &Tag) -> bool {
    field.schema.outer() == (tag.class, tag.number)
}

fn check_sequence(t: &Tlv<'_>, fields: &[Field<'_>], cx: Context) -> Result<(), TypedError> {
    let mut next = 0;
    for c in t.children() {
        let c = c?;
        let tag = c.tag();
        let Some(i) = (next..fields.len()).find(|&i| tag_matches(&fields[i], &tag)) else {
            return Err(Mismatch::UnexpectedComponent {
                class: tag.class,
                number: tag.number,
            }
            .into());
        };
        if fields[next..i]
            .iter()
            .any(|f| matches!(f.presence, Presence::Required))
        {
            return Err(Mismatch::MissingRequired.into());
        }
        check_component(&c, &fields[i], cx)?;
        next = i + 1;
    }
    if fields[next..]
        .iter()
        .any(|f| matches!(f.presence, Presence::Required))
    {
        return Err(Mismatch::MissingRequired.into());
    }
    Ok(())
}

fn check_set(t: &Tlv<'_>, fields: Option<&[Field<'_>]>, cx: Context) -> Result<(), TypedError> {
    if fields.is_some_and(|f| f.len() > MAX_SET_FIELDS) {
        return Err(Mismatch::Capacity.into());
    }
    let mut prev: Option<(u8, u64)> = None;
    let mut seen: u64 = 0;
    for c in t.children() {
        let c = c?;
        let tag = c.tag();
        // X.690 §10.3 with X.680 §8.6: universal, application, context, private; then number.
        let key = (tag.class.bits(), tag.number);
        if cx.mode.is_der() && prev.is_some_and(|p| key < p) {
            return fail(ErrorClass::SetOrder, c.offset());
        }
        prev = Some(key);
        if let Some(fields) = fields {
            let Some(i) =
                (0..fields.len()).find(|&i| seen & (1 << i) == 0 && tag_matches(&fields[i], &tag))
            else {
                return Err(Mismatch::UnexpectedComponent {
                    class: tag.class,
                    number: tag.number,
                }
                .into());
            };
            seen |= 1 << i;
            check_component(&c, &fields[i], cx)?;
        }
    }
    if let Some(fields) = fields {
        if fields
            .iter()
            .enumerate()
            .any(|(i, f)| seen & (1 << i) == 0 && matches!(f.presence, Presence::Required))
        {
            return Err(Mismatch::MissingRequired.into());
        }
    }
    Ok(())
}

fn check_component(c: &Tlv<'_>, f: &Field<'_>, cx: Context) -> Result<(), TypedError> {
    check(c, &f.schema, cx)?;
    // X.690 §11.5: DER leaves out a component equal to its DEFAULT.
    if let Presence::DefaultInt(d) = f.presence {
        if cx.mode.is_der() && integer_value(c, &f.schema) == Some(d) {
            return fail(ErrorClass::DefaultEncoded, c.offset());
        }
    }
    Ok(())
}

fn integer_value(t: &Tlv<'_>, s: &Schema<'_>) -> Option<i128> {
    match s {
        Schema::Prim(Prim::Integer) => integer::decode(t.content(), Mode::Ber).ok()?.to_i128(),
        Schema::Tagged {
            explicit: false,
            inner,
            ..
        } => integer_value(t, inner),
        Schema::Tagged {
            explicit: true,
            inner,
            ..
        } => integer_value(&t.children().next()?.ok()?, inner),
        _ => None,
    }
}

/// The primitive leaves of a string in either form, in value order, without allocating. A segment of a
/// constructed string is an encoding with the universal tag `segment` (X.690 §8.6.4.1, §8.7.3.2, §8.23.6),
/// itself possibly constructed. It nests at most [`MAX_SEGMENT_DEPTH`] levels.
///
/// After the first fault the iterator ends and [`Leaves::fault`] holds it. A caller that needs the leaves
/// twice runs a clone to the end first and reads the fault, then hands the other clone to a content
/// function.
#[derive(Clone, Debug)]
pub struct Leaves<'a> {
    segment: u64,
    /// A primitive element is its own single leaf.
    own: Option<&'a [u8]>,
    stack: [Option<Children<'a>>; MAX_SEGMENT_DEPTH],
    depth: usize,
    fault: Option<TypedError>,
}

impl<'a> Leaves<'a> {
    /// The leaves of `t`, whose segments carry the universal tag number `segment`.
    #[must_use]
    pub fn new(t: &Tlv<'a>, segment: u64) -> Self {
        let mut it = Self {
            segment,
            own: None,
            stack: [const { None }; MAX_SEGMENT_DEPTH],
            depth: 0,
            fault: None,
        };
        if t.is_constructed() {
            it.stack[0] = Some(t.children());
            it.depth = 1;
        } else {
            it.own = Some(t.content());
        }
        it
    }

    /// The first fault met, once the iterator has ended.
    #[must_use]
    pub fn fault(&self) -> Option<TypedError> {
        self.fault
    }

    /// Run a clone to the end: the first fault, or `Ok` if every segment is well formed.
    ///
    /// # Errors
    /// `bad_contents` at the offset of a segment with the wrong universal tag, a reader fault inside the
    /// segments, or [`Mismatch::Capacity`] beyond [`MAX_SEGMENT_DEPTH`].
    pub fn check(&self) -> Result<(), TypedError> {
        let mut probe = self.clone();
        for _ in probe.by_ref() {}
        probe.fault.map_or(Ok(()), Err)
    }
}

impl<'a> Iterator for Leaves<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        if let Some(own) = self.own.take() {
            return Some(own);
        }
        while self.fault.is_none() && self.depth > 0 {
            let children = self.stack[self.depth - 1].as_mut()?;
            let Some(item) = children.next() else {
                self.stack[self.depth - 1] = None;
                self.depth -= 1;
                continue;
            };
            let c = match item {
                Ok(c) => c,
                Err(e) => {
                    self.fault = Some(e.into());
                    return None;
                }
            };
            let tag = c.tag();
            if tag.class != Class::Universal || tag.number != self.segment {
                self.fault = Some(TypedError::Class(Error::new(
                    ErrorClass::BadContents,
                    c.offset(),
                )));
                return None;
            }
            if c.is_constructed() {
                if self.depth == MAX_SEGMENT_DEPTH {
                    self.fault = Some(Mismatch::Capacity.into());
                    return None;
                }
                self.stack[self.depth] = Some(c.children());
                self.depth += 1;
            } else {
                return Some(c.content());
            }
        }
        None
    }
}

/// Validate the contents of a primitive-contents type. X.690 §8, §10, §11 by type; see each module.
///
/// # Errors
/// The class the content function reports, or a fault in a constructed string's segments.
pub fn check_prim(t: &Tlv<'_>, p: Prim, cx: Context) -> Result<(), TypedError> {
    let mode = cx.mode;
    let content = t.content();
    match p {
        Prim::Boolean => drop(boolean::decode(content, mode)?),
        Prim::Integer | Prim::Enumerated => drop(integer::decode(content, mode)?),
        Prim::Null => null::decode(content, mode)?,
        Prim::Oid => drop(oid::ObjectIdentifier::decode(content, mode)?),
        Prim::RelOid => drop(oid::RelativeOid::decode(content, mode)?),
        Prim::OidIri => drop(oid::OidIri::decode(content, mode)?),
        Prim::RelOidIri => drop(oid::RelativeOidIri::decode(content, mode)?),
        Prim::Real => drop(real::decode(content, mode)?),
        Prim::Date => drop(time::parse_date(content)?),
        Prim::TimeOfDay => drop(time::parse_time_of_day(content)?),
        Prim::DateTime => drop(time::parse_date_time(content)?),
        Prim::Duration => time::validate_duration(content, mode)?,
        Prim::Time => time::validate_time(content, mode)?,
        Prim::BitString { named } => {
            if t.is_constructed() {
                let leaves = Leaves::new(t, 3);
                leaves.check()?;
                bitstring::ConstructedBitString::new(leaves, mode)?;
            } else {
                bitstring::BitString::decode(content, mode, named)?;
            }
        }
        Prim::OctetString => {
            if t.is_constructed() {
                let leaves = Leaves::new(t, 4);
                leaves.check()?;
                octetstring::ConstructedOctetString::new(leaves, mode)?;
            }
        }
        Prim::Str(st) => {
            let leaves = Leaves::new(t, 4);
            leaves.check()?;
            strings::validate_segments(st, leaves)?;
        }
        Prim::Utc | Prim::Generalized => {
            let mut buf = [0u8; MAX_TIME_OCTETS];
            let whole = join(&Leaves::new(t, 4), &mut buf)?;
            if matches!(p, Prim::Utc) {
                time::parse_utc_time(whole, mode, cx.pivot)?;
            } else {
                time::parse_generalized_time(whole, mode)?;
            }
        }
    }
    Ok(())
}

/// Join the leaves into `buf`, or report the fault in them.
fn join<'b>(leaves: &Leaves<'_>, buf: &'b mut [u8]) -> Result<&'b [u8], TypedError> {
    leaves.check()?;
    let mut n = 0;
    for seg in leaves.clone() {
        let end = n + seg.len();
        let Some(dst) = buf.get_mut(n..end) else {
            return Err(Mismatch::Capacity.into());
        };
        dst.copy_from_slice(seg);
        n = end;
    }
    Ok(&buf[..n])
}

#[cfg(test)]
mod tests {
    use super::*;

    const INT: Schema<'static> = Schema::Prim(Prim::Integer);
    const OCTETS: Schema<'static> = Schema::Prim(Prim::OctetString);

    fn run(input: &[u8], schema: &Schema<'_>, mode: Mode) -> Result<(), TypedError> {
        validate(input, schema, Context::new(mode), &Limits::DEFAULT)
    }

    fn class(r: Result<(), TypedError>) -> Option<ErrorClass> {
        match r {
            Err(TypedError::Class(e)) => Some(e.class),
            _ => None,
        }
    }

    #[test]
    fn identifier_form_is_judged_before_the_length() {
        // SEQUENCE with the primitive bit, and a length that overruns: the form wins (stream order).
        let seq = Schema::Sequence(None);
        assert_eq!(
            class(run(&[0x10, 0x05, 0x00], &seq, Mode::Ber)),
            Some(ErrorClass::WrongForm)
        );
        // BOOLEAN with the constructed bit.
        let b = Schema::Prim(Prim::Boolean);
        assert_eq!(
            class(run(&[0x21, 0x01, 0xFF], &b, Mode::Ber)),
            Some(ErrorClass::WrongForm)
        );
    }

    #[test]
    fn a_constructed_string_is_ber_only() {
        // [0x24 04] OCTET STRING constructed holding one segment `04 01 AA`.
        let enc = [0x24, 0x03, 0x04, 0x01, 0xAA];
        assert_eq!(run(&enc, &OCTETS, Mode::Ber), Ok(()));
        assert_eq!(
            class(run(&enc, &OCTETS, Mode::Der)),
            Some(ErrorClass::ConstructedString)
        );
    }

    #[test]
    fn a_wrong_tag_is_a_mismatch_not_an_error_class() {
        assert!(matches!(
            run(&[0x04, 0x00], &INT, Mode::Ber),
            Err(TypedError::Mismatch(Mismatch::Tag { .. }))
        ));
    }

    #[test]
    fn a_segment_with_the_wrong_tag_is_bad_contents() {
        // Constructed OCTET STRING whose segment is an INTEGER.
        let enc = [0x24, 0x03, 0x02, 0x01, 0x05];
        assert_eq!(
            class(run(&enc, &OCTETS, Mode::Ber)),
            Some(ErrorClass::BadContents)
        );
    }

    #[test]
    fn leaves_follow_nested_segments_in_order() {
        // 24 0A { 04 01 11, 24 05 { 04 01 22, 04 01 33 } }
        let enc = [
            0x24, 0x0B, 0x04, 0x01, 0x11, 0x24, 0x06, 0x04, 0x01, 0x22, 0x04, 0x01, 0x33,
        ];
        let t = Tlv::parse_exact(&enc, Mode::Ber, &Limits::DEFAULT).unwrap();
        let mut it = Leaves::new(&t, 4);
        assert_eq!(it.next(), Some(&[0x11][..]));
        assert_eq!(it.next(), Some(&[0x22][..]));
        assert_eq!(it.next(), Some(&[0x33][..]));
        assert_eq!(it.next(), None);
        assert_eq!(it.fault(), None);
    }

    #[test]
    fn segment_nesting_beyond_the_bound_is_a_capacity_mismatch() {
        // MAX_SEGMENT_DEPTH + 1 nested empty constructed OCTET STRINGs, each header `24 len`.
        let levels = MAX_SEGMENT_DEPTH + 1;
        let mut enc = [0u8; 2 * (MAX_SEGMENT_DEPTH + 1)];
        for level in 0..levels {
            enc[2 * level] = 0x24;
            enc[2 * level + 1] = u8::try_from(2 * (levels - 1 - level)).unwrap();
        }
        let limits = Limits {
            max_depth: 64,
            ..Limits::DEFAULT
        };
        assert_eq!(
            validate(&enc, &OCTETS, Context::new(Mode::Ber), &limits),
            Err(TypedError::Mismatch(Mismatch::Capacity))
        );
    }

    #[test]
    fn der_checks_set_of_order_with_zero_padding() {
        // SET OF INTEGER: 02 01 05 then 02 01 03 is descending.
        let set_of = Schema::SetOf(Some(&INT));
        let desc = [0x31, 0x06, 0x02, 0x01, 0x05, 0x02, 0x01, 0x03];
        assert_eq!(run(&desc, &set_of, Mode::Ber), Ok(()));
        assert_eq!(
            class(run(&desc, &set_of, Mode::Der)),
            Some(ErrorClass::SetofOrder)
        );
        // A shorter encoding that is a prefix is padded with zeros: 02 01 05 vs 02 01 05 00 is equal-padded.
        assert!(!setof_greater(&[2, 1, 5], &[2, 1, 5, 0]));
        assert!(setof_greater(&[2, 1, 5, 1], &[2, 1, 5]));
    }

    #[test]
    fn der_checks_set_component_order() {
        let set = Schema::Set(None);
        // Context [1] before universal INTEGER.
        let bad = [0x31, 0x06, 0x81, 0x01, 0x00, 0x02, 0x01, 0x00];
        assert_eq!(run(&bad, &set, Mode::Ber), Ok(()));
        assert_eq!(
            class(run(&bad, &set, Mode::Der)),
            Some(ErrorClass::SetOrder)
        );
    }

    #[test]
    fn der_refuses_a_component_equal_to_its_default() {
        const FIELDS: [Field<'static>; 1] = [Field {
            schema: INT,
            presence: Presence::DefaultInt(5),
        }];
        let seq = Schema::Sequence(Some(&FIELDS));
        let five = [0x30, 0x03, 0x02, 0x01, 0x05];
        let six = [0x30, 0x03, 0x02, 0x01, 0x06];
        assert_eq!(run(&five, &seq, Mode::Ber), Ok(()));
        assert_eq!(
            class(run(&five, &seq, Mode::Der)),
            Some(ErrorClass::DefaultEncoded)
        );
        assert_eq!(run(&six, &seq, Mode::Der), Ok(()));
    }

    #[test]
    fn a_missing_required_component_is_a_mismatch() {
        const FIELDS: [Field<'static>; 1] = [Field {
            schema: INT,
            presence: Presence::Required,
        }];
        assert_eq!(
            run(&[0x30, 0x00], &Schema::Sequence(Some(&FIELDS)), Mode::Ber),
            Err(TypedError::Mismatch(Mismatch::MissingRequired))
        );
    }

    #[test]
    fn an_explicit_tag_holds_exactly_one_encoding() {
        let explicit = Schema::Tagged {
            class: Class::ContextSpecific,
            number: 0,
            explicit: true,
            inner: &INT,
        };
        assert_eq!(
            run(&[0xA0, 0x03, 0x02, 0x01, 0x07], &explicit, Mode::Der),
            Ok(())
        );
        assert_eq!(
            run(&[0xA0, 0x00], &explicit, Mode::Der),
            Err(TypedError::Mismatch(Mismatch::ExplicitArity))
        );
    }

    #[test]
    fn a_constructed_time_is_joined_up_to_the_bound() {
        // UTCTime "250101000000Z" split over two segments in BER.
        let enc = [
            0x37, 0x11, 0x04, 0x06, b'2', b'5', b'0', b'1', b'0', b'1', 0x04, 0x07, b'0', b'0',
            b'0', b'0', b'0', b'0', b'Z',
        ];
        assert_eq!(run(&enc, &Schema::Prim(Prim::Utc), Mode::Ber), Ok(()));
    }

    #[test]
    fn a_constructed_time_longer_than_the_bound_is_a_capacity_mismatch() {
        // GeneralizedTime constructed, one OCTET STRING segment of MAX_TIME_OCTETS + 1 digits.
        let mut enc = [b'0'; 4 + MAX_TIME_OCTETS + 1];
        enc[0] = 0x38;
        enc[1] = u8::try_from(2 + MAX_TIME_OCTETS + 1).unwrap();
        enc[2] = 0x04;
        enc[3] = u8::try_from(MAX_TIME_OCTETS + 1).unwrap();
        assert_eq!(
            run(&enc, &Schema::Prim(Prim::Generalized), Mode::Ber),
            Err(TypedError::Mismatch(Mismatch::Capacity))
        );
    }

    #[test]
    fn trailing_data_comes_last() {
        assert_eq!(
            class(run(&[0x02, 0x01, 0x00, 0xFF], &INT, Mode::Ber)),
            Some(ErrorClass::TrailingData)
        );
    }
}
