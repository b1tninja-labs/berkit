//! The typed adapter: where the TLV layer and the content layers meet. Test-only for now.
//!
//! The crate has a content function per type and a TLV reader that never looks at the type
//! (`docs/design/structured.md` section 5). This module does the type-dependent work in between:
//!
//! 1. the IDENTIFIER decides the form before any length fault (`wrong_form`; in DER `constructed_string`),
//! 2. the reader parses and validates the structure,
//! 3. the content function of the matching module runs with the mode and flags the type needs
//!    (named-bit list, `UTCTime` century pivot, string alphabet),
//! 4. constructed strings are flattened to their segments, DER SET and SET OF order is checked, and a
//!    DEFAULT component that is present is refused in DER.
//!
//! A type is a [`Schema`]. The corpus types of `corpus/types.md` and the X.690 Annex A and 8.14 types are
//! written from the clauses the cases cite.

use super::json::Json;
use super::model::Node;
use berkit::strings::{self, StringType};
use berkit::tag::{Class, Tag};
use berkit::tlv::{self, Limits, Tlv};
use berkit::{
    Error, ErrorClass, Mode, bitstring, boolean, integer, null, octetstring, oid, real, time,
};

/// Why a typed read or write did not produce a value.
#[derive(Debug)]
pub enum Failure {
    /// A corpus error class: the encoding is refused.
    Class(Error),
    /// The adapter cannot express the case (no error class exists for it): a tag that does not match the
    /// schema, a missing required component, a value outside what the encoders take.
    Unsupported(String),
}

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        Failure::Class(e)
    }
}

impl From<&str> for Failure {
    fn from(s: &str) -> Self {
        Failure::Unsupported(s.to_string())
    }
}

fn fail<T>(class: ErrorClass, offset: usize) -> Result<T, Failure> {
    Err(Failure::Class(Error::new(class, offset)))
}

fn unsupported<T>(why: impl Into<String>) -> Result<T, Failure> {
    Err(Failure::Unsupported(why.into()))
}

// ---------------------------------------------------------------- schemas

/// A universal type with primitive contents rules.
#[derive(Clone, Copy, Debug)]
enum Prim {
    Boolean,
    Integer,
    Enumerated,
    /// `named` is true for a type defined with a `NamedBitList` (X.680 section 22.7).
    BitString {
        named: bool,
    },
    OctetString,
    Null,
    Oid,
    RelOid,
    Real,
    Utc,
    Generalized,
    Date,
    TimeOfDay,
    DateTime,
    Duration,
    Time,
    OidIri,
    RelOidIri,
    Str(StringType),
}

impl Prim {
    fn number(self) -> u64 {
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

    /// Types that may be constructed in BER (X.690 section 8.6.1, 8.7.1, 8.23.6, 8.25); DER takes the
    /// primitive form only (section 10.2).
    fn may_be_constructed(self) -> bool {
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

#[derive(Clone, Debug)]
enum Presence {
    Required,
    Optional,
    /// `DEFAULT n` on an INTEGER (X.690 section 11.5).
    DefaultInt(i128),
}

#[derive(Clone, Debug)]
struct Field {
    schema: Schema,
    presence: Presence,
}

#[derive(Clone, Debug)]
enum Schema {
    Prim(Prim),
    /// SEQUENCE; `None` leaves the components unchecked (the case names only the outer type).
    Sequence(Option<Vec<Field>>),
    SequenceOf(Box<Schema>),
    /// SET; DER checks the tag order (X.690 section 10.3).
    Set(Option<Vec<Field>>),
    /// SET OF; DER checks the encoding order (X.690 section 11.6).
    SetOf(Option<Box<Schema>>),
    Tagged {
        class: Class,
        number: u64,
        explicit: bool,
        inner: Box<Schema>,
    },
}

impl Schema {
    /// The outer identifier (class, number) a value of this type carries.
    fn outer(&self) -> (Class, u64) {
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

#[derive(PartialEq, Eq)]
enum Form {
    Primitive,
    Constructed,
    /// Either in BER, primitive only in DER.
    Either,
}

fn tagged(class: Class, number: u64, explicit: bool, inner: Schema) -> Schema {
    Schema::Tagged {
        class,
        number,
        explicit,
        inner: Box::new(inner),
    }
}

fn field(schema: Schema, presence: Presence) -> Field {
    Field { schema, presence }
}

fn string_type(name: &str) -> Option<StringType> {
    Some(match name {
        "NumericString" => StringType::Numeric,
        "PrintableString" => StringType::Printable,
        "VisibleString" => StringType::Visible,
        "IA5String" => StringType::Ia5,
        "UTF8String" => StringType::Utf8,
        "BMPString" => StringType::Bmp,
        "UniversalString" => StringType::Universal,
        "TeletexString" => StringType::Teletex,
        "VideotexString" => StringType::Videotex,
        "GraphicString" => StringType::Graphic,
        "GeneralString" => StringType::General,
        _ => return None,
    })
}

fn universal(name: &str) -> Option<Schema> {
    let p = match name {
        "BOOLEAN" => Prim::Boolean,
        "INTEGER" => Prim::Integer,
        "ENUMERATED" => Prim::Enumerated,
        "BIT STRING" => Prim::BitString { named: false },
        "OCTET STRING" => Prim::OctetString,
        "NULL" => Prim::Null,
        "OBJECT IDENTIFIER" => Prim::Oid,
        "RELATIVE-OID" => Prim::RelOid,
        "REAL" => Prim::Real,
        "UTCTime" => Prim::Utc,
        "GeneralizedTime" => Prim::Generalized,
        "DATE" => Prim::Date,
        "TIME-OF-DAY" => Prim::TimeOfDay,
        "DATE-TIME" => Prim::DateTime,
        "DURATION" => Prim::Duration,
        "TIME" => Prim::Time,
        "OID-IRI" => Prim::OidIri,
        "RELATIVE-OID-IRI" => Prim::RelOidIri,
        other => Prim::Str(string_type(other)?),
    };
    Some(Schema::Prim(p))
}

/// Application 1, `Name ::= [APPLICATION 1] IMPLICIT SEQUENCE { VisibleString x3 }` (X.690 A.1).
fn annex_name() -> Schema {
    let vs = || {
        field(
            Schema::Prim(Prim::Str(StringType::Visible)),
            Presence::Required,
        )
    };
    tagged(
        Class::Application,
        1,
        false,
        Schema::Sequence(Some(vec![vs(), vs(), vs()])),
    )
}

/// `Date ::= [APPLICATION 3] IMPLICIT VisibleString` (X.690 A.1).
fn annex_date() -> Schema {
    tagged(
        Class::Application,
        3,
        false,
        Schema::Prim(Prim::Str(StringType::Visible)),
    )
}

fn ctx_explicit(n: u64, inner: Schema) -> Schema {
    tagged(Class::ContextSpecific, n, true, inner)
}

/// The schema a case's `type` names. The universal names, the types of `corpus/types.md`, the types of
/// X.690 8.14 and Annex A.1 (the notes in `docs/design/annex-examples.md` paraphrase the latter).
fn schema_for(ty: &str) -> Option<Schema> {
    if let Some(s) = universal(ty) {
        return Some(s);
    }
    let ctx = Class::ContextSpecific;
    Some(match ty {
        "SEQUENCE" => Schema::Sequence(None),
        _ if ty.starts_with("SEQUENCE {") => Schema::Sequence(None),
        "SET" => Schema::Set(None),
        "SET OF" => Schema::SetOf(None),
        "corpus:NamedBitString" => Schema::Prim(Prim::BitString { named: true }),
        "corpus:ExplicitInt2" => tagged(ctx, 2, true, Schema::Prim(Prim::Integer)),
        "corpus:ImplicitInt2" => tagged(ctx, 2, false, Schema::Prim(Prim::Integer)),
        "corpus:DefaultSeq" => Schema::Sequence(Some(vec![field(
            Schema::Prim(Prim::Integer),
            Presence::DefaultInt(5),
        )])),
        "corpus:DefaultSeqTagged" => Schema::Sequence(Some(vec![
            field(
                tagged(ctx, 0, false, Schema::Prim(Prim::Integer)),
                Presence::DefaultInt(5),
            ),
            field(
                tagged(ctx, 1, false, Schema::Prim(Prim::Integer)),
                Presence::Optional,
            ),
        ])),
        // X.690 8.14: Type1 ::= VisibleString; Type2 ::= [APPLICATION 3] IMPLICIT Type1;
        // Type3 ::= [2] Type2; Type4 ::= [APPLICATION 7] IMPLICIT Type3; Type5 ::= [2] IMPLICIT Type2.
        "x690:8.14:Type1" => Schema::Prim(Prim::Str(StringType::Visible)),
        "x690:8.14:Type2" => tagged(Class::Application, 3, false, universal("VisibleString")?),
        "x690:8.14:Type3" => tagged(ctx, 2, true, schema_for("x690:8.14:Type2")?),
        "x690:8.14:Type4" => tagged(Class::Application, 7, false, schema_for("x690:8.14:Type3")?),
        "x690:8.14:Type5" => tagged(ctx, 2, false, schema_for("x690:8.14:Type2")?),
        "x690:A.1:PersonnelRecord" => {
            let req = |s| field(s, Presence::Required);
            let child_info = Schema::Set(Some(vec![
                req(annex_name()),
                req(ctx_explicit(0, annex_date())),
            ]));
            tagged(
                Class::Application,
                0,
                false,
                Schema::Set(Some(vec![
                    req(annex_name()),
                    req(ctx_explicit(0, universal("VisibleString")?)),
                    req(tagged(
                        Class::Application,
                        2,
                        false,
                        Schema::Prim(Prim::Integer),
                    )),
                    req(ctx_explicit(1, annex_date())),
                    req(ctx_explicit(2, annex_name())),
                    // DEFAULT {} on the SEQUENCE OF: its DER rule is not exercised by a case.
                    field(
                        tagged(ctx, 3, false, Schema::SequenceOf(Box::new(child_info))),
                        Presence::Optional,
                    ),
                ])),
            )
        }
        _ => return None,
    })
}

// ---------------------------------------------------------------- reading

struct Cx {
    mode: Mode,
    /// The century pivot for `UTCTime`: the case's policy `utctime-pivot` leaves it to the caller.
    pivot: u8,
}

/// Read `input` as one encoding of type `ty` (or untyped) in `mode`, and return its tree.
///
/// Stream order: a fault in the identifier (form) comes before the length octets, which come before the
/// contents, and trailing data comes last.
pub fn decode(
    input: &[u8],
    ty: Option<&str>,
    mode: Mode,
    limits: &Limits,
) -> Result<Node, Failure> {
    let schema = match ty {
        None => None,
        Some(t) => match schema_for(t) {
            Some(s) => Some(s),
            None => return unsupported(format!("unknown type {t}")),
        },
    };
    let cx = Cx {
        mode,
        pivot: time::RFC5280_PIVOT,
    };
    if let Some(s) = &schema {
        // The identifier is judged before the length. A fault in the identifier itself is left to the reader.
        if let Ok((tag, _)) = Tag::decode(input) {
            if !tag.is_universal_zero() {
                check_identifier(&tag, s, &cx)?;
            }
        }
    }
    let (t, rest) = Tlv::parse(input, mode, limits)?;
    t.validate()?;
    if let Some(s) = &schema {
        check_content(&t, s, &cx)?;
    }
    if !rest.is_empty() {
        return fail(ErrorClass::TrailingData, t.encoding().len());
    }
    Ok(Node::from_tlv(&t)?)
}

fn check_identifier(tag: &Tag, schema: &Schema, cx: &Cx) -> Result<(), Failure> {
    let (class, number) = schema.outer();
    if tag.class != class || tag.number != number {
        return unsupported(format!(
            "tag mismatch: schema wants {class:?} {number}, input has {:?} {}",
            tag.class, tag.number
        ));
    }
    match (schema.form(), tag.constructed) {
        (Form::Primitive, true) | (Form::Constructed, false) => fail(ErrorClass::WrongForm, 0),
        (Form::Either, true) if cx.mode.is_der() => fail(ErrorClass::ConstructedString, 0),
        _ => Ok(()),
    }
}

/// An element of a schema: identifier first, then the contents.
fn check(t: &Tlv<'_>, schema: &Schema, cx: &Cx) -> Result<(), Failure> {
    check_identifier(&t.tag(), schema, cx)?;
    check_content(t, schema, cx)
}

fn check_content(t: &Tlv<'_>, schema: &Schema, cx: &Cx) -> Result<(), Failure> {
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
            let kids = children(t)?;
            let [only] = kids.as_slice() else {
                return unsupported("an explicit tag holds exactly one encoding");
            };
            check(only, inner, cx)
        }
        Schema::Sequence(None) => Ok(()),
        Schema::Sequence(Some(fields)) => check_sequence(t, fields, cx),
        Schema::SequenceOf(inner) => {
            for c in children(t)? {
                check(&c, inner, cx)?;
            }
            Ok(())
        }
        Schema::Set(fields) => check_set(t, fields.as_deref(), cx),
        Schema::SetOf(inner) => {
            let mut prev: Option<Tlv<'_>> = None;
            for c in children(t)? {
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

fn children<'a>(t: &Tlv<'a>) -> Result<Vec<Tlv<'a>>, Failure> {
    t.children().map(|c| c.map_err(Failure::from)).collect()
}

/// X.690 section 11.6: shorter encodings are padded with trailing zero octets before comparing.
fn setof_greater(a: &[u8], b: &[u8]) -> bool {
    let n = a.len().max(b.len());
    let pad = |x: &[u8]| {
        let mut v = x.to_vec();
        v.resize(n, 0);
        v
    };
    pad(a) > pad(b)
}

fn tag_matches(field: &Field, tag: &Tag) -> bool {
    field.schema.outer() == (tag.class, tag.number)
}

fn check_sequence(t: &Tlv<'_>, fields: &[Field], cx: &Cx) -> Result<(), Failure> {
    let mut next = 0;
    for c in children(t)? {
        let tag = c.tag();
        let Some(i) = (next..fields.len()).find(|&i| tag_matches(&fields[i], &tag)) else {
            return unsupported(format!(
                "component {:?} {} not in the schema here",
                tag.class, tag.number
            ));
        };
        if fields[next..i]
            .iter()
            .any(|f| matches!(f.presence, Presence::Required))
        {
            return unsupported("a required component is missing");
        }
        check_component(&c, &fields[i], cx)?;
        next = i + 1;
    }
    if fields[next..]
        .iter()
        .any(|f| matches!(f.presence, Presence::Required))
    {
        return unsupported("a required component is missing");
    }
    Ok(())
}

fn check_set(t: &Tlv<'_>, fields: Option<&[Field]>, cx: &Cx) -> Result<(), Failure> {
    let mut prev: Option<(u8, u64)> = None;
    let mut seen = vec![false; fields.map_or(0, <[Field]>::len)];
    for c in children(t)? {
        let tag = c.tag();
        // X.690 section 10.3 with X.680 section 8.6: universal, application, context, private; then number.
        let key = (tag.class.bits(), tag.number);
        if cx.mode.is_der() && prev.is_some_and(|p| key < p) {
            return fail(ErrorClass::SetOrder, c.offset());
        }
        prev = Some(key);
        if let Some(fields) = fields {
            let Some(i) = (0..fields.len()).find(|&i| !seen[i] && tag_matches(&fields[i], &tag))
            else {
                return unsupported(format!(
                    "component {:?} {} not in the SET",
                    tag.class, tag.number
                ));
            };
            seen[i] = true;
            check_component(&c, &fields[i], cx)?;
        }
    }
    if let Some(fields) = fields {
        if fields
            .iter()
            .zip(&seen)
            .any(|(f, s)| !s && matches!(f.presence, Presence::Required))
        {
            return unsupported("a required component is missing");
        }
    }
    Ok(())
}

fn check_component(c: &Tlv<'_>, f: &Field, cx: &Cx) -> Result<(), Failure> {
    check(c, &f.schema, cx)?;
    // X.690 section 11.5: DER leaves out a component equal to its DEFAULT.
    if let Presence::DefaultInt(d) = f.presence {
        if cx.mode.is_der() && integer_value(c, &f.schema) == Some(d) {
            return fail(ErrorClass::DefaultEncoded, c.offset());
        }
    }
    Ok(())
}

fn integer_value(t: &Tlv<'_>, s: &Schema) -> Option<i128> {
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

/// The primitive leaves of a string in either form, in value order. A segment of a constructed string is
/// an encoding with the universal tag `segment` (X.690 section 8.6.4.1, 8.7.3.2, 8.23.6), itself possibly
/// constructed.
fn leaves<'a>(t: &Tlv<'a>, segment: u64, out: &mut Vec<&'a [u8]>) -> Result<(), Failure> {
    if !t.is_constructed() {
        out.push(t.content());
        return Ok(());
    }
    for c in t.children() {
        let c = c?;
        let tag = c.tag();
        if tag.class != Class::Universal || tag.number != segment {
            return fail(ErrorClass::BadContents, c.offset());
        }
        leaves(&c, segment, out)?;
    }
    Ok(())
}

fn check_prim(t: &Tlv<'_>, p: Prim, cx: &Cx) -> Result<(), Failure> {
    let mode = cx.mode;
    let content = t.content();
    match p {
        Prim::Boolean => boolean::decode(content, mode).map(drop)?,
        Prim::Integer | Prim::Enumerated => integer::decode(content, mode).map(drop)?,
        Prim::Null => null::decode(content, mode)?,
        Prim::Oid => oid::ObjectIdentifier::decode(content, mode).map(drop)?,
        Prim::RelOid => oid::RelativeOid::decode(content, mode).map(drop)?,
        Prim::OidIri => oid::OidIri::decode(content, mode).map(drop)?,
        Prim::RelOidIri => oid::RelativeOidIri::decode(content, mode).map(drop)?,
        Prim::Real => real::decode(content, mode).map(drop)?,
        Prim::Date => time::parse_date(content).map(drop)?,
        Prim::TimeOfDay => time::parse_time_of_day(content).map(drop)?,
        Prim::DateTime => time::parse_date_time(content).map(drop)?,
        Prim::Duration => time::validate_duration(content, mode)?,
        Prim::Time => time::validate_time(content, mode)?,
        Prim::BitString { named } => {
            if t.is_constructed() {
                let mut segs = Vec::new();
                leaves(t, 3, &mut segs)?;
                bitstring::ConstructedBitString::new(segs.iter().copied(), mode).map(drop)?;
            } else {
                bitstring::BitString::decode(content, mode, named).map(drop)?;
            }
        }
        Prim::OctetString => {
            if t.is_constructed() {
                let mut segs = Vec::new();
                leaves(t, 4, &mut segs)?;
                octetstring::ConstructedOctetString::new(segs.iter().copied(), mode).map(drop)?;
            }
        }
        Prim::Str(st) => {
            let mut segs = Vec::new();
            leaves(t, 4, &mut segs)?;
            strings::validate_segments(st, segs.iter().copied())?;
        }
        Prim::Utc | Prim::Generalized => {
            let mut segs = Vec::new();
            leaves(t, 4, &mut segs)?;
            let whole: Vec<u8> = segs.concat();
            if matches!(p, Prim::Utc) {
                time::parse_utc_time(&whole, mode, cx.pivot).map(drop)?;
            } else {
                time::parse_generalized_time(&whole, mode).map(drop)?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- writing

fn decimal_to_magnitude(s: &str) -> Result<(bool, Vec<u8>), Failure> {
    let (negative, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return unsupported(format!("not a decimal integer: {s:?}"));
    }
    let mut mag: Vec<u8> = vec![0];
    for d in digits.bytes() {
        let mut carry = u32::from(d - b'0');
        for b in mag.iter_mut().rev() {
            let v = u32::from(*b) * 10 + carry;
            *b = (v & 0xFF) as u8;
            carry = v >> 8;
        }
        while carry > 0 {
            mag.insert(0, (carry & 0xFF) as u8);
            carry >>= 8;
        }
    }
    Ok((negative, mag))
}

fn arcs(value: &str) -> Result<Vec<u128>, Failure> {
    value
        .split('.')
        .map(|a| {
            a.parse::<u128>()
                .map_err(|_| Failure::Unsupported(format!("arc {a:?} does not fit a u128")))
        })
        .collect()
}

fn unhex(s: &str) -> Result<Vec<u8>, Failure> {
    super::model::unhex(s).or_else(unsupported)
}

/// Encode a typed value (`input.typed` of an encode case) as one encoding with the crate's encoders and
/// writer.
pub fn encode_typed(ty: &str, value: &Json) -> Result<Vec<u8>, Failure> {
    let Some(Schema::Prim(p)) = universal(ty) else {
        return unsupported(format!("no typed encoder for {ty}"));
    };
    let mut buf = vec![0u8; 1024];
    let n = match (p, value) {
        (Prim::Boolean, Json::Bool(b)) => boolean::encode(*b, &mut buf).ok_or("boolean"),
        (Prim::Null, Json::Null) => Ok(null::encode(&mut buf)),
        (Prim::Integer | Prim::Enumerated, Json::Str(s)) => {
            let (neg, mag) = decimal_to_magnitude(s)?;
            integer::encode_magnitude(neg, &mag, &mut buf).ok_or("integer")
        }
        (Prim::Oid, Json::Str(s)) => {
            oid::encode_object_identifier_u128(&arcs(s)?, &mut buf).map_err(|_| "object identifier")
        }
        (Prim::RelOid, Json::Str(s)) => {
            oid::encode_relative_oid_u128(&arcs(s)?, &mut buf).map_err(|_| "relative oid")
        }
        (Prim::OctetString, Json::Str(h)) => {
            octetstring::encode(&unhex(h)?, &mut buf).map_err(|_| "octet string")
        }
        (Prim::BitString { .. }, v) => {
            let unused = v
                .get("unused")
                .and_then(Json::as_u64)
                .ok_or("bit string unused")?;
            let data = unhex(
                v.get("hex")
                    .and_then(Json::as_str)
                    .ok_or("bit string hex")?,
            )?;
            let bit_len = (data.len() * 8)
                .checked_sub(usize::try_from(unused).map_err(|_| "unused")?)
                .ok_or("unused")?;
            bitstring::encode(&data, bit_len, &mut buf).map_err(|_| "bit string")
        }
        _ => Err("value form"),
    };
    let n = match n {
        Ok(n) => n,
        Err(what) => return unsupported(format!("encoder refused the {what} value")),
    };
    let tag = Tag::new(Class::Universal, false, p.number());
    let mut out = vec![0u8; n + 16];
    let len = tlv::write_tlv(&tag, &buf[..n], &mut out)
        .map_err(|e| Failure::Unsupported(e.to_string()))?;
    out.truncate(len);
    Ok(out)
}
