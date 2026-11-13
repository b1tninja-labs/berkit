//! The typed adapter for the corpus runner: schemas by corpus type name, and decoded values in the corpus forms.
//!
//! The type-dependent validation itself is the library module `berkit::typed` (identifier and form, structure,
//! SET and SET OF order, DEFAULT, content). This file only names the corpus types as library schemas, calls
//! that module, and renders the decoded value of a bare universal type in the `typed.value` forms of
//! `corpus/format.md`, which needs allocation and so stays out of the library.

use super::json::Json;
use super::model::Node;
use berkit::strings::StringType;
use berkit::tag::{Class, Tag};
use berkit::tlv::{self, Limits, Tlv};
use berkit::typed::{self, Context, Field, Presence, Prim, Schema, TypedError};
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
    /// A crate accessor contradicts the octets it was built from (for example `Integer::to_i64` disagrees with
    /// the value read directly from the contents).
    Accessor(String),
}

/// A successful typed read: the tree, and the decoded value in the `typed.value` forms of `corpus/format.md`
/// when the type has a value form (`None` for the others).
#[derive(Debug)]
pub struct Decoded {
    pub node: Node,
    pub value: Option<Json>,
}

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        Failure::Class(e)
    }
}

impl From<TypedError> for Failure {
    fn from(e: TypedError) -> Self {
        match e {
            TypedError::Class(c) => Failure::Class(c),
            TypedError::Mismatch(m) => Failure::Unsupported(m.to_string()),
        }
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

/// Schemas borrow from each other; the runner builds a handful per process, so leaking them is the simplest
/// way to get `'static` references in test code.
fn leak<T>(t: T) -> &'static T {
    Box::leak(Box::new(t))
}

fn tagged(class: Class, number: u64, explicit: bool, inner: Schema<'static>) -> Schema<'static> {
    Schema::Tagged {
        class,
        number,
        explicit,
        inner: leak(inner),
    }
}

fn field(schema: Schema<'static>, presence: Presence) -> Field<'static> {
    Field { schema, presence }
}

fn fields(v: Vec<Field<'static>>) -> &'static [Field<'static>] {
    Vec::leak(v)
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

fn universal(name: &str) -> Option<Schema<'static>> {
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
fn annex_name() -> Schema<'static> {
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
        Schema::Sequence(Some(fields(vec![vs(), vs(), vs()]))),
    )
}

/// `Date ::= [APPLICATION 3] IMPLICIT VisibleString` (X.690 A.1).
fn annex_date() -> Schema<'static> {
    tagged(
        Class::Application,
        3,
        false,
        Schema::Prim(Prim::Str(StringType::Visible)),
    )
}

fn ctx_explicit(n: u64, inner: Schema<'static>) -> Schema<'static> {
    tagged(Class::ContextSpecific, n, true, inner)
}

/// The schema a case's `type` names. The universal names, the types of `corpus/types.md`, the types of
/// X.690 8.14 and Annex A.1 (the notes in `docs/design/annex-examples.md` paraphrase the latter).
fn schema_for(ty: &str) -> Option<Schema<'static>> {
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
        "corpus:DefaultSeq" => Schema::Sequence(Some(fields(vec![field(
            Schema::Prim(Prim::Integer),
            Presence::DefaultInt(5),
        )]))),
        "corpus:DefaultSeqTagged" => Schema::Sequence(Some(fields(vec![
            field(
                tagged(ctx, 0, false, Schema::Prim(Prim::Integer)),
                Presence::DefaultInt(5),
            ),
            field(
                tagged(ctx, 1, false, Schema::Prim(Prim::Integer)),
                Presence::Optional,
            ),
        ]))),
        // X.690 8.14: Type1 ::= VisibleString; Type2 ::= [APPLICATION 3] IMPLICIT Type1;
        // Type3 ::= [2] Type2; Type4 ::= [APPLICATION 7] IMPLICIT Type3; Type5 ::= [2] IMPLICIT Type2.
        "x690:8.14:Type1" => Schema::Prim(Prim::Str(StringType::Visible)),
        "x690:8.14:Type2" => tagged(Class::Application, 3, false, universal("VisibleString")?),
        "x690:8.14:Type3" => tagged(ctx, 2, true, schema_for("x690:8.14:Type2")?),
        "x690:8.14:Type4" => tagged(Class::Application, 7, false, schema_for("x690:8.14:Type3")?),
        "x690:8.14:Type5" => tagged(ctx, 2, false, schema_for("x690:8.14:Type2")?),
        "x690:A.1:PersonnelRecord" => {
            let req = |s| field(s, Presence::Required);
            let child_info = Schema::Set(Some(fields(vec![
                req(annex_name()),
                req(ctx_explicit(0, annex_date())),
            ])));
            tagged(
                Class::Application,
                0,
                false,
                Schema::Set(Some(fields(vec![
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
                        tagged(ctx, 3, false, Schema::SequenceOf(leak(child_info))),
                        Presence::Optional,
                    ),
                ]))),
            )
        }
        _ => return None,
    })
}

// ---------------------------------------------------------------- reading

/// Read `input` as one encoding of type `ty` (or untyped) in `mode`, and return its tree.
///
/// Stream order: a fault in the identifier (form) comes before the length octets, which come before the
/// contents, and trailing data comes last. The checks are `berkit::typed`; this adds the tree and the value.
pub fn decode(
    input: &[u8],
    ty: Option<&str>,
    mode: Mode,
    limits: &Limits,
    pivot: Option<u8>,
) -> Result<Decoded, Failure> {
    let schema = match ty {
        None => None,
        Some(t) => match schema_for(t) {
            Some(s) => Some(s),
            None => return unsupported(format!("unknown type {t}")),
        },
    };
    let cx = Context {
        mode,
        pivot: pivot.unwrap_or(time::RFC5280_PIVOT),
    };
    if let Some(s) = &schema {
        if let Ok((tag, _)) = Tag::decode(input) {
            if !tag.is_universal_zero() {
                typed::check_identifier(&tag, s, mode)?;
            }
        }
    }
    let (t, rest) = Tlv::parse(input, mode, limits)?;
    t.validate()?;
    let value = match &schema {
        Some(s) => {
            typed::check_content(&t, s, cx)?;
            match s {
                Schema::Prim(p) => render(&t, *p, cx)?,
                _ => None,
            }
        }
        None => None,
    };
    if !rest.is_empty() {
        return fail(ErrorClass::TrailingData, t.encoding().len());
    }
    Ok(Decoded {
        node: Node::from_tlv(&t)?,
        value,
    })
}

/// The primitive leaves of a string in either form, in value order, collected for the value forms.
fn leaves<'a>(t: &Tlv<'a>, segment: u64) -> Vec<&'a [u8]> {
    typed::Leaves::new(t, segment).collect()
}

/// The decoded value of a primitive-contents type, in the corpus forms (`corpus/format.md`, `typed.value`).
/// The contents were validated by `berkit::typed::check_content`; this decodes them again for the value.
/// `None` for a type with no value form: DATE, TIME-OF-DAY, DATE-TIME, DURATION, TIME, OID-IRI,
/// RELATIVE-OID-IRI and the unchecked 8-bit strings.
fn render(t: &Tlv<'_>, p: Prim, cx: Context) -> Result<Option<Json>, Failure> {
    let mode = cx.mode;
    let content = t.content();
    Ok(match p {
        Prim::Boolean => Some(Json::Bool(boolean::decode(content, mode)?)),
        Prim::Integer | Prim::Enumerated => Some(Json::Str(values::integer(&integer::decode(
            content, mode,
        )?)?)),
        Prim::Null => {
            null::decode(content, mode)?;
            Some(Json::Null)
        }
        Prim::Oid => {
            let o = oid::ObjectIdentifier::decode(content, mode)?;
            Some(Json::Str(values::arcs(o.arcs())?))
        }
        Prim::RelOid => {
            let o = oid::RelativeOid::decode(content, mode)?;
            Some(Json::Str(values::arcs(o.arcs())?))
        }
        Prim::OidIri
        | Prim::RelOidIri
        | Prim::Date
        | Prim::TimeOfDay
        | Prim::DateTime
        | Prim::Duration
        | Prim::Time => None,
        Prim::Real => Some(values::real(&real::decode(content, mode)?)),
        Prim::BitString { named } => {
            if t.is_constructed() {
                let segs = leaves(t, 3);
                let b = bitstring::ConstructedBitString::new(segs.iter().copied(), mode)?;
                let mut data = vec![0u8; b.octet_len()];
                b.copy_data(&mut data)?;
                Some(values::bit_string(b.unused_bits(), &data))
            } else {
                let b = bitstring::BitString::decode(content, mode, named)?;
                // The unused bits are not part of the value and read as zero (format.md).
                let mut data = b.data().to_vec();
                if let Some(last) = data.last_mut() {
                    *last &= 0xFFu8 << b.unused_bits();
                }
                Some(values::bit_string(b.unused_bits(), &data))
            }
        }
        Prim::OctetString => {
            if t.is_constructed() {
                let segs = leaves(t, 4);
                let o = octetstring::ConstructedOctetString::new(segs.iter().copied(), mode)?;
                let mut data = vec![0u8; o.len()];
                o.copy_into(&mut data)?;
                Some(Json::Str(super::model::hex(&data)))
            } else {
                Some(Json::Str(super::model::hex(octetstring::decode(content))))
            }
        }
        Prim::Str(st) => values::string(st, &leaves(t, 4).concat())?,
        Prim::Utc | Prim::Generalized => {
            let whole: Vec<u8> = leaves(t, 4).concat();
            if matches!(p, Prim::Utc) {
                Some(values::time(&time::parse_utc_time(&whole, mode, cx.pivot)?))
            } else {
                Some(values::time(&time::parse_generalized_time(&whole, mode)?))
            }
        }
    })
}

/// Decoded values in the `typed.value` forms of `corpus/format.md` (revision 1.2). Written from the corpus
/// forms; the crate supplies the parsed pieces and the conversions here are plain `std`.
mod values {
    use super::{Failure, Json};
    use berkit::integer::Integer;
    use berkit::oid::Arc;
    use berkit::real::{Kind, Real};
    use berkit::strings::StringType;
    use berkit::time::{Fields, Zone};

    fn num<T: ToString>(n: &T) -> Json {
        Json::Num(n.to_string())
    }

    fn obj(members: Vec<(&str, Json)>) -> Json {
        Json::Obj(
            members
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    /// A big-endian unsigned number as decimal digits.
    pub fn magnitude_to_decimal(mag: &[u8]) -> String {
        let mut n: Vec<u8> = mag.to_vec();
        let mut digits = Vec::new();
        while n.iter().any(|&b| b != 0) {
            let mut rem = 0u32;
            for b in &mut n {
                let cur = rem * 256 + u32::from(*b);
                *b = u8::try_from(cur / 10).expect("a base-256 digit");
                rem = cur % 10;
            }
            digits.push(b'0' + u8::try_from(rem).expect("a decimal digit"));
        }
        if digits.is_empty() {
            digits.push(b'0');
        }
        digits.reverse();
        String::from_utf8(digits).expect("ASCII digits")
    }

    /// The magnitude of a two's-complement number, read directly from its octets.
    fn twos_complement(bytes: &[u8]) -> (bool, Vec<u8>) {
        if bytes[0] & 0x80 == 0 {
            return (false, bytes.to_vec());
        }
        let mut m: Vec<u8> = bytes.iter().map(|b| !b).collect();
        for b in m.iter_mut().rev() {
            let (v, carry) = b.overflowing_add(1);
            *b = v;
            if !carry {
                break;
            }
        }
        (true, m)
    }

    fn signed(negative: bool, mag: &[u8]) -> String {
        let d = magnitude_to_decimal(mag);
        if negative && d != "0" {
            format!("-{d}")
        } else {
            d
        }
    }

    /// Compare an accessor's answer with what the digits say: `Some(digits)` when the value fits, else `None`.
    fn agree(what: &str, got: Option<&str>, digits: &str, fits: bool) -> Result<(), Failure> {
        let expect = fits.then_some(digits);
        if got == expect {
            Ok(())
        } else {
            Err(Failure::Accessor(format!(
                "{what} gave {got:?} for {digits}"
            )))
        }
    }

    /// An INTEGER or ENUMERATED as a decimal string of any size. The crate's accessors are checked against
    /// the digits read directly from the octets: `is_negative` with `write_magnitude`, and `to_i64`,
    /// `to_u64`, `to_i128` and `to_u128` must all tell the same story.
    pub fn integer(i: &Integer<'_>) -> Result<String, Failure> {
        let (neg, mag) = twos_complement(i.as_bytes());
        let direct = signed(neg, &mag);
        let mut buf = vec![0u8; i.as_bytes().len()];
        let n = i
            .write_magnitude(&mut buf)
            .ok_or_else(|| Failure::Accessor("Integer::write_magnitude gave None".into()))?;
        let via_crate = signed(i.is_negative(), &buf[..n]);
        if via_crate != direct {
            return Err(Failure::Accessor(format!(
                "is_negative/write_magnitude give {via_crate}, the octets are {direct}"
            )));
        }
        agree(
            "Integer::to_i64",
            i.to_i64().map(|v| v.to_string()).as_deref(),
            &direct,
            direct.parse::<i64>().is_ok(),
        )?;
        agree(
            "Integer::to_u64",
            i.to_u64().map(|v| v.to_string()).as_deref(),
            &direct,
            direct.parse::<u64>().is_ok(),
        )?;
        agree(
            "Integer::to_i128",
            i.to_i128().map(|v| v.to_string()).as_deref(),
            &direct,
            direct.parse::<i128>().is_ok(),
        )?;
        agree(
            "Integer::to_u128",
            i.to_u128().map(|v| v.to_string()).as_deref(),
            &direct,
            direct.parse::<u128>().is_ok(),
        )?;
        Ok(direct)
    }

    fn arc(a: &Arc<'_>) -> Result<String, Failure> {
        let mut buf = vec![0u8; a.octets().len()];
        let n = a
            .be_bytes(&mut buf)
            .ok_or_else(|| Failure::Accessor("Arc::be_bytes gave None".into()))?;
        let dec = magnitude_to_decimal(&buf[..n]);
        agree(
            "Arc::to_u64",
            a.to_u64().map(|v| v.to_string()).as_deref(),
            &dec,
            dec.parse::<u64>().is_ok(),
        )?;
        agree(
            "Arc::to_u128",
            a.to_u128().map(|v| v.to_string()).as_deref(),
            &dec,
            dec.parse::<u128>().is_ok(),
        )?;
        Ok(dec)
    }

    /// The arcs of an OBJECT IDENTIFIER or RELATIVE-OID as a dotted string with arcs of any size.
    pub fn arcs<'a>(it: impl Iterator<Item = Arc<'a>>) -> Result<String, Failure> {
        let parts: Result<Vec<String>, Failure> = it.map(|a| arc(&a)).collect();
        Ok(parts?.join("."))
    }

    pub fn bit_string(unused: u8, data: &[u8]) -> Json {
        obj(vec![
            ("unused", num(&unused)),
            ("hex", Json::Str(super::super::model::hex(data))),
        ])
    }

    /// The classification of the first contents octet and the contents, with no float (policy `real-opaque`).
    pub fn real(r: &Real<'_>) -> Json {
        let kind = match r.kind() {
            Kind::Zero => "zero",
            Kind::PlusInfinity => "plus-infinity",
            Kind::MinusInfinity => "minus-infinity",
            Kind::NotANumber => "not-a-number",
            Kind::MinusZero => "minus-zero",
            Kind::Binary { .. } => "binary",
            Kind::Decimal { .. } => "decimal",
        };
        obj(vec![
            ("kind", Json::Str(kind.into())),
            ("hex", Json::Str(super::super::model::hex(r.contents()))),
        ])
    }

    /// The characters of a string whose alphabet the crate checks; `None` for the unchecked 8-bit strings.
    pub fn string(st: StringType, octets: &[u8]) -> Result<Option<Json>, Failure> {
        let bad = |what: &str| Failure::Accessor(format!("validated {what} octets do not decode"));
        let text = match st {
            StringType::Numeric
            | StringType::Printable
            | StringType::Visible
            | StringType::Ia5
            | StringType::Utf8 => String::from_utf8(octets.to_vec()).map_err(|_| bad("UTF-8"))?,
            StringType::Bmp => {
                let units = octets
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]));
                char::decode_utf16(units)
                    .collect::<Result<String, _>>()
                    .map_err(|_| bad("BMPString"))?
            }
            StringType::Universal => octets
                .chunks_exact(4)
                .map(|c| char::from_u32(u32::from_be_bytes([c[0], c[1], c[2], c[3]])))
                .collect::<Option<String>>()
                .ok_or_else(|| bad("UniversalString"))?,
            _ => return Ok(None),
        };
        Ok(Some(Json::Str(text)))
    }

    /// The parsed fields of a `UTCTime` or `GeneralizedTime`: year, month, day, hour, then minute, second,
    /// fraction (digits), comma and zone where written.
    pub fn time(f: &Fields<'_>) -> Json {
        let mut m = vec![
            ("year", num(&f.year)),
            ("month", num(&f.month)),
            ("day", num(&f.day)),
            ("hour", num(&f.hour)),
        ];
        if let Some(v) = f.minute {
            m.push(("minute", num(&v)));
        }
        if let Some(v) = f.second {
            m.push(("second", num(&v)));
        }
        if !f.fraction.is_empty() {
            m.push((
                "fraction",
                Json::Str(String::from_utf8_lossy(f.fraction).into_owned()),
            ));
            if f.comma {
                m.push(("comma", Json::Bool(true)));
            }
        }
        match f.zone {
            Zone::Local => {}
            Zone::Utc => m.push(("zone", Json::Str("Z".into()))),
            Zone::Offset {
                negative,
                hour,
                minute,
                minute_present,
            } => {
                let sign = if negative { '-' } else { '+' };
                let text = if minute_present {
                    format!("{sign}{hour:02}:{minute:02}")
                } else {
                    format!("{sign}{hour:02}")
                };
                m.push(("zone", Json::Str(text)));
            }
        }
        obj(m)
    }
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
