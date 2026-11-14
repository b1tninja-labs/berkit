//! ASN.1 BER and DER reading and writing, built clause by clause from ITU-T X.690 (02/2021) and X.680.
//!
//! `no_std`, no allocation, no dependencies. The standard is the reference: `docs/design/` holds the
//! clause-by-clause notes each module was built from, and `corpus/` the language-neutral conformance cases.
//! Every module is a content-level or TLV-level function over caller-supplied slices.
//!
//! # Example
//!
//! Read `SEQUENCE { INTEGER 5 }` in DER, then show that an INTEGER with a redundant leading octet is refused.
//!
//! ```
//! use berkit::tlv::{Limits, Tlv};
//! use berkit::{integer, ErrorClass, Mode};
//!
//! // 30 03 | 02 01 05: a SEQUENCE of three content octets holding the INTEGER 5.
//! let der = [0x30, 0x03, 0x02, 0x01, 0x05];
//! let seq = Tlv::parse_exact(&der, Mode::Der, &Limits::DEFAULT)?;
//! assert!(seq.is_constructed());
//!
//! let child = seq.children().next().expect("one child")?;
//! let n = integer::decode(child.content(), Mode::Der)?;
//! assert_eq!(n.to_i64(), Some(5));
//!
//! // Contents 00 05 (the INTEGER 5 with a redundant leading zero octet) are not minimal, X.690 §8.3.2.
//! let padded = integer::decode(&[0x00, 0x05], Mode::Der);
//! assert_eq!(padded.unwrap_err().class, ErrorClass::NonMinimalInteger);
//! # Ok::<(), berkit::Error>(())
//! ```

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
