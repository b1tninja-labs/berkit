//! REAL contents, carried unchanged with a structural classifier. X.690 §8.5, §11.3.
//!
//! POLICY (scope of version 1): the contents octets are carried opaquely and losslessly. There is no
//! conversion to a floating-point type and no decimal parsing, because X.680 §21.3 makes base-2 and base-10
//! values distinct abstract values and an `f64` cannot hold all of them. [`decode`] classifies the first octet
//! (§8.5.6), checks the structure that octet implies, and in DER the §11.3 rules that need no arithmetic. A
//! [`Real`] writes back exactly the octets it was decoded from.
//!
//! ISO 6093, which §8.5.8 relies on for the decimal forms, is not in the corpus. Only what §8.5.8 and §11.3.2
//! state is checked; the rest is `[unread]` in the roadmap.

use crate::{Error, ErrorClass, Mode};

/// The base of a binary encoding. X.690 §8.5.7.2 (`11` is reserved and refused).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Base {
    /// Bits 6 to 5 are `00`.
    Two,
    /// Bits 6 to 5 are `01`.
    Eight,
    /// Bits 6 to 5 are `10`.
    Sixteen,
}

/// The ISO 6093 number representation of a decimal encoding. X.690 §8.5.8.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecimalForm {
    /// Bits 6 to 1 are `000001`.
    Nr1,
    /// Bits 6 to 1 are `000010`.
    Nr2,
    /// Bits 6 to 1 are `000011`.
    Nr3,
}

/// What the first contents octet says the value is. X.690 §8.5.6. Borrowed parts are views into the contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind<'a> {
    /// No contents octets: plus zero. X.690 §8.5.2.
    Zero,
    /// `40`. X.690 §8.5.9.
    PlusInfinity,
    /// `41`. X.690 §8.5.9.
    MinusInfinity,
    /// `42`. X.690 §8.5.9.
    NotANumber,
    /// `43`. X.690 §8.5.3, §8.5.9.
    MinusZero,
    /// Binary encoding, bit 8 set. X.690 §8.5.7. The value is S x N x 2^F x B^E.
    Binary {
        /// Bit 7: S is -1. X.690 §8.5.7.1.
        negative: bool,
        /// Bits 6 to 5. X.690 §8.5.7.2.
        base: Base,
        /// Bits 4 to 3, the binary scaling factor F, 0 to 3. X.690 §8.5.7.3.
        scale: u8,
        /// The exponent E: big-endian two's-complement octets (at least one). X.690 §8.5.7.4.
        exponent: &'a [u8],
        /// The mantissa N: big-endian unsigned octets, possibly none. X.690 §8.5.7.5.
        mantissa: &'a [u8],
    },
    /// Decimal encoding, bits 8 to 7 `00`. X.690 §8.5.8. The ISO 6093 field is not parsed.
    Decimal {
        /// Bits 6 to 1.
        form: DecimalForm,
        /// The ISO 6093 field: every octet after the first.
        field: &'a [u8],
    },
}

/// A decoded REAL: its contents octets, unchanged, and their classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Real<'a> {
    contents: &'a [u8],
    kind: Kind<'a>,
}

impl<'a> Real<'a> {
    /// The contents octets, exactly as decoded.
    #[must_use]
    pub const fn contents(&self) -> &'a [u8] {
        self.contents
    }

    /// The classification of the first contents octet.
    #[must_use]
    pub const fn kind(&self) -> Kind<'a> {
        self.kind
    }

    /// Write the contents octets back unchanged. Returns the octets written, or `None` if `out` is too small.
    #[must_use]
    pub fn write(&self, out: &mut [u8]) -> Option<usize> {
        out.get_mut(..self.contents.len())?
            .copy_from_slice(self.contents);
        Some(self.contents.len())
    }
}

/// The four contents octets of the special values. X.690 §8.5.9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Special {
    /// `40`.
    PlusInfinity,
    /// `41`.
    MinusInfinity,
    /// `42`.
    NotANumber,
    /// `43`.
    MinusZero,
}

/// Write the contents of a special value: one octet. X.690 §8.5.9. Returns 1, or `None` if `out` is empty.
#[must_use]
pub fn encode_special(value: Special, out: &mut [u8]) -> Option<usize> {
    *out.first_mut()? = match value {
        Special::PlusInfinity => 0x40,
        Special::MinusInfinity => 0x41,
        Special::NotANumber => 0x42,
        Special::MinusZero => 0x43,
    };
    Some(1)
}

/// Decode the contents octets of a REAL.
///
/// Structure checked in both modes, all reported as `bad_contents`:
/// - bits 8 to 7 `01` (X.690 §8.5.9): exactly one octet, `40` to `43`;
/// - decimal (§8.5.8): bits 6 to 1 are `000001`, `000010` or `000011`;
/// - binary (§8.5.7): bits 6 to 5 are not `11` (§8.5.7.2); the octets the exponent format implies are present
///   (§8.5.7.4); in format `11` the length X is at least one and, when X >= 2, the first nine bits of the exponent
///   are neither all zeros nor all ones.
///
/// Empty contents are plus zero (§8.5.2).
///
/// In DER the §11.3 checks that need no arithmetic then apply, reported as `real_form`:
/// - decimal: the first octet is `03` (NR3), no SPACE, a mantissa without a zero first or last digit, `.` then
///   `E`, and an exponent written `+0` or with a non-zero first digit and no plus sign (§11.3.2.1 to §11.3.2.6);
/// - binary: base 2 and F = 0 (§11.3.1); N non-empty, odd and without a leading `00` octet (§11.3.1, §8.5.7.5
///   NOTE 1); the exponent in the fewest octets.
///
/// POLICY (derived, not stated): in DER, exponent format `11` is refused unless X is at least 4, because a shorter
/// exponent has a shorter format and one value should have one encoding (§7.4). POLICY (not stated, X.690 does
/// not address it): in BER a binary encoding with no mantissa octets is accepted; DER refuses it as N is not odd.
/// POLICY: in BER the decimal field is accepted without inspection (ISO 6093 is not read).
///
/// # Errors
/// `bad_contents` or, in DER only, `real_form`; the offset is the octet of the first violation.
pub fn decode(content: &[u8], mode: Mode) -> Result<Real<'_>, Error> {
    let bad = |offset| Error::new(ErrorClass::BadContents, offset);
    let Some((&first, rest)) = content.split_first() else {
        return Ok(Real {
            contents: content,
            kind: Kind::Zero,
        });
    };
    let kind = match first >> 6 {
        0b01 => {
            if !rest.is_empty() {
                return Err(bad(1));
            }
            match first {
                0x40 => Kind::PlusInfinity,
                0x41 => Kind::MinusInfinity,
                0x42 => Kind::NotANumber,
                0x43 => Kind::MinusZero,
                _ => return Err(bad(0)),
            }
        }
        0b00 => {
            let form = match first & 0x3F {
                1 => DecimalForm::Nr1,
                2 => DecimalForm::Nr2,
                3 => DecimalForm::Nr3,
                _ => return Err(bad(0)),
            };
            Kind::Decimal { form, field: rest }
        }
        _ => binary(content, first, rest)?,
    };
    if mode.is_der() {
        der_check(content, kind)?;
    }
    Ok(Real {
        contents: content,
        kind,
    })
}

/// Split a binary encoding (bit 8 set) into its fields, checking the structure. X.690 §8.5.7.
fn binary<'a>(content: &'a [u8], first: u8, rest: &'a [u8]) -> Result<Kind<'a>, Error> {
    let bad = |offset| Error::new(ErrorClass::BadContents, offset);
    let base = match (first >> 4) & 3 {
        0 => Base::Two,
        1 => Base::Eight,
        2 => Base::Sixteen,
        _ => return Err(bad(0)),
    };
    let (exp_len, exp_at) = if first & 3 < 3 {
        (usize::from(first & 3) + 1, 1)
    } else {
        match rest.first() {
            Some(&x) if x != 0 => (usize::from(x), 2),
            _ => return Err(bad(1)),
        }
    };
    let Some(exponent) = content.get(exp_at..exp_at + exp_len) else {
        return Err(bad(content.len()));
    };
    // §8.5.7.4 d): the first nine bits of a form-11 exponent are not all zeros or all ones. With one octet
    // there are only eight bits: the standard is silent, and the encoding is accepted.
    if first & 3 == 3 && exp_len >= 2 && redundant(exponent[0], exponent[1]) {
        return Err(bad(exp_at));
    }
    Ok(Kind::Binary {
        negative: first & 0x40 != 0,
        base,
        scale: (first >> 2) & 3,
        exponent,
        mantissa: &content[exp_at + exp_len..],
    })
}

/// The first octet and bit 8 of the second are all zero or all ones.
const fn redundant(a: u8, b: u8) -> bool {
    (a == 0x00 && b & 0x80 == 0) || (a == 0xFF && b & 0x80 != 0)
}

/// The DER restrictions of X.690 §11.3 that need no arithmetic.
fn der_check(content: &[u8], kind: Kind<'_>) -> Result<(), Error> {
    let form = |offset| Error::new(ErrorClass::RealForm, offset);
    match kind {
        Kind::Binary {
            base,
            scale,
            exponent,
            mantissa,
            ..
        } => {
            if base != Base::Two || scale != 0 {
                return Err(form(0));
            }
            let exp_at = content.len() - mantissa.len() - exponent.len();
            // Exponent in the fewest octets: no redundant leading octet; format 11 only from four octets.
            if exponent.len() >= 2 && redundant(exponent[0], exponent[1]) {
                return Err(form(exp_at));
            }
            if content[0] & 3 == 3 && exponent.len() < 4 {
                return Err(form(1));
            }
            let mant_at = content.len() - mantissa.len();
            match mantissa {
                [] | [0x00, ..] => Err(form(mant_at)),
                [.., last] if last & 1 == 0 => Err(form(content.len() - 1)),
                _ => Ok(()),
            }
        }
        Kind::Decimal { form: f, field } => {
            if f != DecimalForm::Nr3 {
                return Err(form(0));
            }
            check_nr3(field).map_err(|at| form(at + 1))
        }
        _ => Ok(()),
    }
}

/// X.690 §11.3.2.2 to §11.3.2.6 on the octets after the first. `Err` carries the offset within `field`.
fn check_nr3(field: &[u8]) -> Result<(), usize> {
    if let Some(at) = field.iter().position(|&b| b == b' ') {
        return Err(at); // §11.3.2.2
    }
    let sign = usize::from(field.first() == Some(&b'-')); // §11.3.2.3
    let digits = field[sign..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    let mant_end = sign + digits;
    if digits == 0 {
        return Err(sign); // a negative value or not, it must begin with a digit
    }
    if field[sign] == b'0' {
        return Err(sign); // §11.3.2.4 first digit
    }
    if field[mant_end - 1] == b'0' {
        return Err(mant_end - 1); // §11.3.2.4 last digit
    }
    if field.get(mant_end..mant_end + 2) != Some(b".E") {
        return Err(mant_end); // §11.3.2.5
    }
    let exp = &field[mant_end + 2..];
    if exp == b"+0" {
        return Ok(()); // §11.3.2.6
    }
    let neg = usize::from(exp.first() == Some(&b'-'));
    let ds = &exp[neg..];
    if ds.is_empty() || !ds.iter().all(u8::is_ascii_digit) || ds[0] == b'0' {
        return Err(mant_end + 2);
    }
    Ok(())
}
