//! NULL contents. X.690 §8.8.
//!
//! The contents octets shall not contain any octets (§8.8.2).

use crate::{Error, ErrorClass, Mode};

/// Decode the contents octets of a NULL: they must be empty. X.690 §8.8.2.
///
/// BER and DER agree, so `mode` does not change the result.
///
/// # Errors
/// `bad_contents` if any contents octet is present.
pub fn decode(content: &[u8], _mode: Mode) -> Result<(), Error> {
    if content.is_empty() {
        Ok(())
    } else {
        Err(Error::new(ErrorClass::BadContents, 0))
    }
}

/// Write the contents of a NULL: no octets. X.690 §8.8.2. Returns 0.
#[must_use]
pub const fn encode(_out: &mut [u8]) -> usize {
    0
}
