//! BOOLEAN contents. X.690 §8.2, §11.1.
//!
//! The contents are exactly one octet: `00` is FALSE (§8.2.2), any other value is TRUE in BER (§8.2.2, a
//! sender's option), and DER narrows TRUE to `FF` (§11.1). Offsets count from the start of the contents.

use crate::{Error, ErrorClass, Mode};

/// Decode the contents octets of a BOOLEAN.
///
/// A length other than one is `bad_contents` (X.690 §8.2.1). `00` is FALSE (§8.2.2). In BER every other octet
/// is TRUE (§8.2.2); in DER only `FF` is, anything else is `boolean_not_ff` (§11.1).
///
/// # Errors
/// `bad_contents` for a length other than one, `boolean_not_ff` in DER for a non-zero octet other than `FF`.
pub fn decode(content: &[u8], mode: Mode) -> Result<bool, Error> {
    let [octet] = *content else {
        return Err(Error::new(ErrorClass::BadContents, 0));
    };
    match octet {
        0x00 => Ok(false),
        0xFF => Ok(true),
        _ if mode.is_der() => Err(Error::new(ErrorClass::BooleanNotFf, 0)),
        _ => Ok(true),
    }
}

/// Write the contents octet of a BOOLEAN: `FF` for TRUE, `00` for FALSE. X.690 §8.2.2, §11.1.
///
/// `FF` is the one TRUE value both BER and DER accept. Returns the number of octets written (1), or `None`
/// when `out` is empty.
#[must_use]
pub fn encode(value: bool, out: &mut [u8]) -> Option<usize> {
    *out.first_mut()? = if value { 0xFF } else { 0x00 };
    Some(1)
}
