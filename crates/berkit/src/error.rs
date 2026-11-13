//! The error type shared by every module.
//!
//! The classes are the fixed list of the conformance corpus (`corpus/format.md`, "Error classes"), so a
//! corpus case names a class and a module returns the same class. X.690 has no receiver-side error rules:
//! every class here is derived from a sender-side "shall", and each doc comment cites that clause.
//! Where the standard states no clause, the class says so.

use core::fmt;

/// Why an encoding was refused. The first thirteen are refused in both BER and DER; the rest in DER only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorClass {
    /// Input ends inside the identifier, the length octets, or the declared contents. X.690 §8.1.1.1, §8.1.3.3.
    Truncated,
    /// The initial length octet is `FF`, which the long form must not use. X.690 §8.1.3.5 c).
    ReservedLength,
    /// A primitive identifier with the indefinite length `80`. X.690 §8.1.3.2 a), §8.1.3.6.
    IndefiniteOnPrimitive,
    /// A child's header or contents extend past the end of its enclosing contents. X.690 §8.1.3.3, §8.1.4.
    LengthOverrun,
    /// An indefinite-length encoding ends without `00 00`. X.690 §8.1.3.6.2, §8.1.5.
    MissingEoc,
    /// A tag number 0 to 30 in the high form, or a leading zero subsequent octet. X.690 §8.1.2.2, §8.1.2.4.2 c).
    NonMinimalTag,
    /// A length, tag number or nesting depth beyond the limit the caller declared. No clause: the standard
    /// sets no bound (X.690 §8.1.3.5 allows up to 126 length octets).
    LimitExceeded,
    /// The constructed bit contradicts the type. X.690 §8.2.1, §8.3.1, §8.8.1, §8.9.1, §8.1.2.5.
    WrongForm,
    /// Contents break the type's rule. X.690 §8.2.1, §8.8.2, §8.6.2.2.
    BadContents,
    /// INTEGER or ENUMERATED contents longer than the smallest two's-complement form. X.690 §8.3.2.
    NonMinimalInteger,
    /// An OID or RELATIVE-OID subidentifier starting with octet `80`. X.690 §8.19.2, §8.20.2.
    NonMinimalSubidentifier,
    /// Octets remain after the single top-level encoding. Derived from X.690 §8.1.1.1; reporting leftover
    /// input is a design decision of this crate.
    TrailingData,
    /// End-of-contents octets, or the UNIVERSAL 0 identifier, where no indefinite-length encoding is open.
    /// X.690 §8.1.5 ¶1; X.680 Table 1 reserves UNIVERSAL 0 for the encoding rules.
    StrayEoc,
    /// DER: a definite length not in the fewest octets. X.690 §10.1 (BER allows it, §8.1.3.5 NOTE 2).
    NonMinimalLength,
    /// DER: the indefinite length form. X.690 §10.1.
    IndefiniteLength,
    /// DER: a constructed BIT STRING, OCTET STRING or restricted character string. X.690 §10.2.
    ConstructedString,
    /// DER: BOOLEAN TRUE with a contents octet other than `FF`. X.690 §11.1.
    BooleanNotFf,
    /// DER: unused bits of a BIT STRING not zero. X.690 §11.2.1.
    UnusedBitsNonzero,
    /// DER: trailing 0 bits not removed where X.680 §22.7 applies. X.690 §11.2.2.
    BitstringTrailingZeros,
    /// DER: SET components not in canonical tag order. X.690 §10.3, X.680 §8.6.
    SetOrder,
    /// DER: SET OF encodings not in ascending order. X.690 §11.6.
    SetofOrder,
    /// DER: a component equal to its DEFAULT is present. X.690 §11.5.
    DefaultEncoded,
    /// DER: a time not in the X.690 §11.7 or §11.8 form.
    TimeFormat,
    /// DER: a REAL not in the X.690 §11.3 form.
    RealForm,
}

impl ErrorClass {
    /// Every class, in the order of the corpus format.
    pub const ALL: [Self; 24] = [
        Self::Truncated,
        Self::ReservedLength,
        Self::IndefiniteOnPrimitive,
        Self::LengthOverrun,
        Self::MissingEoc,
        Self::NonMinimalTag,
        Self::LimitExceeded,
        Self::WrongForm,
        Self::BadContents,
        Self::NonMinimalInteger,
        Self::NonMinimalSubidentifier,
        Self::TrailingData,
        Self::StrayEoc,
        Self::NonMinimalLength,
        Self::IndefiniteLength,
        Self::ConstructedString,
        Self::BooleanNotFf,
        Self::UnusedBitsNonzero,
        Self::BitstringTrailingZeros,
        Self::SetOrder,
        Self::SetofOrder,
        Self::DefaultEncoded,
        Self::TimeFormat,
        Self::RealForm,
    ];

    /// The class name used in the corpus, such as `non_minimal_length`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truncated => "truncated",
            Self::ReservedLength => "reserved_length",
            Self::IndefiniteOnPrimitive => "indefinite_on_primitive",
            Self::LengthOverrun => "length_overrun",
            Self::MissingEoc => "missing_eoc",
            Self::NonMinimalTag => "non_minimal_tag",
            Self::LimitExceeded => "limit_exceeded",
            Self::WrongForm => "wrong_form",
            Self::BadContents => "bad_contents",
            Self::NonMinimalInteger => "non_minimal_integer",
            Self::NonMinimalSubidentifier => "non_minimal_subidentifier",
            Self::TrailingData => "trailing_data",
            Self::StrayEoc => "stray_eoc",
            Self::NonMinimalLength => "non_minimal_length",
            Self::IndefiniteLength => "indefinite_length",
            Self::ConstructedString => "constructed_string",
            Self::BooleanNotFf => "boolean_not_ff",
            Self::UnusedBitsNonzero => "unused_bits_nonzero",
            Self::BitstringTrailingZeros => "bitstring_trailing_zeros",
            Self::SetOrder => "set_order",
            Self::SetofOrder => "setof_order",
            Self::DefaultEncoded => "default_encoded",
            Self::TimeFormat => "time_format",
            Self::RealForm => "real_form",
        }
    }

    /// The class with this corpus name, or `None`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    /// True for the classes only DER refuses (BER accepts the encoding).
    #[must_use]
    pub const fn der_only(self) -> bool {
        matches!(
            self,
            Self::NonMinimalLength
                | Self::IndefiniteLength
                | Self::ConstructedString
                | Self::BooleanNotFf
                | Self::UnusedBitsNonzero
                | Self::BitstringTrailingZeros
                | Self::SetOrder
                | Self::SetofOrder
                | Self::DefaultEncoded
                | Self::TimeFormat
                | Self::RealForm
        )
    }
}

/// An error: the class and the octet offset of the first violation.
///
/// The offset is advisory. It counts from the start of the slice the function was given (a content
/// function counts from the start of the contents), and a corpus case does not have to match it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    /// The class of the first violation in stream order.
    pub class: ErrorClass,
    /// The octet offset of that violation.
    pub offset: usize,
}

impl Error {
    /// An error of `class` at `offset`.
    #[must_use]
    pub const fn new(class: ErrorClass, offset: usize) -> Self {
        Self { class, offset }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at octet {}", self.class.name(), self.offset)
    }
}

impl core::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_round_trips_through_its_corpus_name() {
        for class in ErrorClass::ALL {
            assert_eq!(ErrorClass::from_name(class.name()), Some(class));
        }
        assert_eq!(ErrorClass::from_name("no_such_class"), None);
    }

    #[test]
    fn the_thirteen_shared_classes_come_first_and_the_der_only_ones_after() {
        let (shared, der) = ErrorClass::ALL.split_at(13);
        assert!(shared.iter().all(|c| !c.der_only()));
        assert!(der.iter().all(|c| c.der_only()));
    }

    #[test]
    fn corpus_names_are_unique() {
        for (i, a) in ErrorClass::ALL.iter().enumerate() {
            for b in &ErrorClass::ALL[i + 1..] {
                assert_ne!(a.name(), b.name());
            }
        }
    }
}
