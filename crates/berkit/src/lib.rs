//! ASN.1 BER and DER reading and writing, built clause by clause from ITU-T X.690 (02/2021) and X.680.
//!
//! `no_std`, no allocation, no dependencies. The standard is the reference: `docs/design/` holds the
//! clause-by-clause notes each module was built from, and `corpus/` the language-neutral conformance cases.
//! Every module is a content-level or TLV-level function over caller-supplied slices.

#![no_std]

pub mod bitstring;
pub mod boolean;
pub mod error;
pub mod integer;
pub mod length;
pub mod null;
pub mod octetstring;
pub mod oid;
pub mod real;
pub mod strings;
pub mod tag;
pub mod time;
pub mod tlv;
pub mod typed;

pub use error::{Error, ErrorClass};

/// Which encoding rules a function applies. X.690 §8 (BER) and §10, §11 (DER).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Basic encoding rules: the value rules of §8 and the framing of §8.1, accepting the sender's options.
    Ber,
    /// Distinguished encoding rules: BER restricted by §10 and §11, so each value has one encoding.
    Der,
}

impl Mode {
    /// True for DER.
    #[must_use]
    pub const fn is_der(self) -> bool {
        matches!(self, Self::Der)
    }
}
