//! `UTCTime`, `GeneralizedTime` and the TIME types. X.690 §8.25, §8.26, §11.7 to §11.9; X.680 §38, §46, §47.
//!
//! Every function takes the CONTENTS octets of one value (the framing module handles the identifier, which for
//! DATE, `TIME-OF-DAY`, `DATE-TIME` and DURATION is `1F 1F`, `1F 20`, `1F 21`, `1F 22`, universal tags 31 to 34).
//! Offsets count from the start of the contents.
//!
//! Two classes of error are returned. [`ErrorClass::BadContents`]: the octets are not a value of the type
//! (grammar or range). [`ErrorClass::TimeFormat`]: a valid BER value that DER refuses (X.690 §11.7, §11.8,
//! §11.9). BER mode accepts every option the grammar gives the sender.
//!
//! Not in the corpus, so `[unread]`: ISO 8601, which X.680 §46.3 and §47.3 cite for the exact forms. Everything
//! below that depends on it is POLICY and says so: the range of the leap second, the range of the digits of a
//! time difference, a decimal sign with no digit after it, the days of each month.

use crate::{Error, ErrorClass, Mode};

/// The century pivot of RFC 5280 §4.1.2.5.1: YY of 50 or more is 19YY, YY below 50 is 20YY.
///
/// Neither X.680 §47.3 nor X.690 gives a rule for the century of the two-digit year, so every function that
/// parses a `UTCTime` takes the pivot as a parameter. This constant is the PKIX profile's choice, offered for
/// callers who want it; it is not a rule of the standard.
pub const RFC5280_PIVOT: u8 = 50;

/// Whether the constructed form is allowed for `UTCTime` and `GeneralizedTime` in `mode`.
///
/// POLICY: the same as for restricted strings, [`crate::strings::check_form`] (BER accepts, DER refuses with
/// [`ErrorClass::ConstructedString`]). The standard does not say it directly: X.690 §10.2 names "restricted
/// character string types", and the two time types are `IMPLICIT VisibleString` only through X.690 §8.25. DATE,
/// `TIME-OF-DAY`, `DATE-TIME`, DURATION and TIME are always primitive (X.690 §8.26.1.1 to §8.26.5.1), so the framing
/// agent must refuse a constructed form for them in both modes with [`ErrorClass::WrongForm`].
///
/// # Errors
/// [`ErrorClass::ConstructedString`].
pub const fn check_form(mode: Mode, constructed: bool) -> Result<(), Error> {
    crate::strings::check_form(mode, constructed)
}

const fn bad(offset: usize) -> Error {
    Error::new(ErrorClass::BadContents, offset)
}

const fn der(offset: usize) -> Error {
    Error::new(ErrorClass::TimeFormat, offset)
}

/// The zone designator of a `UTCTime` or `GeneralizedTime`. X.680 §46.3 a) to c), §47.3 c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// No designator: local time of day (`GeneralizedTime` only, X.680 §46.3 a)).
    Local,
    /// The character `Z`: UTC. X.680 §46.3 b), §47.3 c) 1).
    Utc,
    /// A signed difference from UTC. The time is local; UTC is the local time minus the difference. X.680 §46.3
    /// c), §47.3 c) 2), and a positive difference means local time is ahead of UTC (§46.2 NOTE).
    Offset {
        /// The sign was `-`.
        negative: bool,
        /// Hours of the difference.
        hour: u8,
        /// Minutes of the difference (0 when omitted).
        minute: u8,
        /// The minutes were written (a `GeneralizedTime` may omit them, X.680 §46.3 c); a `UTCTime` never does).
        minute_present: bool,
    },
}

impl Zone {
    /// The difference in minutes, positive when local time is ahead of UTC; `None` for [`Zone::Local`], 0 for `Z`.
    #[must_use]
    pub const fn minutes_east(self) -> Option<i16> {
        match self {
            Self::Local => None,
            Self::Utc => Some(0),
            Self::Offset {
                negative,
                hour,
                minute,
                ..
            } => {
                let m = hour as i16 * 60 + minute as i16;
                Some(if negative { -m } else { m })
            }
        }
    }
}

/// The parsed fields of a `UTCTime` or `GeneralizedTime`. X.680 §46.3, §47.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fields<'a> {
    /// The full year. For `UTCTime` the century comes from the caller's pivot.
    pub year: u16,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31 (see [`Fields::is_calendar_date`] for the days of each month).
    pub day: u8,
    /// 0 to 23.
    pub hour: u8,
    /// `None` when the precision is the hour (`GeneralizedTime` only).
    pub minute: Option<u8>,
    /// `None` when the precision is the hour or the minute.
    pub second: Option<u8>,
    /// The ASCII digits after the decimal sign, a fraction of the last unit present. Empty when none. X.680 §46.3.
    pub fraction: &'a [u8],
    /// The decimal sign was a comma (X.680 §46.3 a) 2); DER allows the full stop only, X.690 §11.7.4).
    pub comma: bool,
    /// The zone designator.
    pub zone: Zone,
}

impl Fields<'_> {
    /// True if the day exists in the month of the (proleptic Gregorian) year.
    ///
    /// Not applied by the parsers: X.680 §47.3 states only "DD (01 to 31)" and the calendar rules come from
    /// ISO 8601 (`[unread]`). A caller that wants real dates calls this.
    #[must_use]
    pub const fn is_calendar_date(&self) -> bool {
        calendar_ok(self.year, self.month, self.day)
    }
}

const fn calendar_ok(year: u16, month: u8, day: u8) -> bool {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    day >= 1 && day <= max
}

struct Cursor<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn peek_digit(&self) -> bool {
        self.peek().is_some_and(|b| b.is_ascii_digit())
    }

    fn digit(&mut self) -> Result<u8, Error> {
        match self.peek() {
            Some(b) if b.is_ascii_digit() => {
                self.pos += 1;
                Ok(b - b'0')
            }
            _ => Err(bad(self.pos)),
        }
    }

    /// Two digits; the error is at the first one that is not a digit.
    fn two(&mut self) -> Result<u8, Error> {
        let a = self.digit()?;
        let b = self.digit()?;
        Ok(a * 10 + b)
    }

    /// Two digits within `lo..=hi`; a range error is at the first digit.
    fn two_in(&mut self, lo: u8, hi: u8) -> Result<u8, Error> {
        let at = self.pos;
        let v = self.two()?;
        if v < lo || v > hi {
            Err(bad(at))
        } else {
            Ok(v)
        }
    }

    fn done(&self) -> Result<(), Error> {
        if self.pos == self.s.len() {
            Ok(())
        } else {
            Err(bad(self.pos))
        }
    }
}

/// POLICY (`[unread]` ISO 8601): the digits of a time difference are hours 00 to 23 and minutes 00 to 59.
fn offset_zone(c: &mut Cursor<'_>, minutes_optional: bool) -> Result<Zone, Error> {
    let negative = c.peek() == Some(b'-');
    c.pos += 1; // the sign, checked by the caller
    let hour = c.two_in(0, 23)?;
    let (minute, minute_present) = if minutes_optional && c.pos == c.s.len() {
        (0, false)
    } else {
        (c.two_in(0, 59)?, true)
    };
    Ok(Zone::Offset {
        negative,
        hour,
        minute,
        minute_present,
    })
}

fn is_sign(b: u8) -> bool {
    b == b'+' || b == b'-'
}

/// Parse `UTCTime` contents. X.680 §47.3 (BER); in DER also X.690 §11.8.
///
/// `YYMMDDhhmm`, optional `ss`, then `Z` or a sign and `hhmm`. `pivot` fixes the century: a two-digit year of
/// `pivot` or more is 19YY, less is 20YY (the standard gives no rule; [`RFC5280_PIVOT`] is the PKIX choice).
/// Ranges: month 01 to 12, day 01 to 31, hour 00 to 23, minute and second 00 to 59 (X.680 §47.3 a), b); hour 24
/// is excluded, so midnight is `000000` of the following day, X.690 §11.8.3). The time difference digits are
/// POLICY (see [`Zone`]).
///
/// DER (X.690 §11.8.1, §11.8.2) additionally requires the seconds and the zone `Z`: [`ErrorClass::TimeFormat`]
/// at the offset where the seconds should start, or at the zone.
///
/// # Errors
/// [`ErrorClass::BadContents`] for a grammar or range violation, [`ErrorClass::TimeFormat`] for DER.
pub fn parse_utc_time(content: &[u8], mode: Mode, pivot: u8) -> Result<Fields<'_>, Error> {
    let mut c = Cursor { s: content, pos: 0 };
    let yy = c.two()?;
    let month = c.two_in(1, 12)?;
    let day = c.two_in(1, 31)?;
    let hour = c.two_in(0, 23)?;
    let minute = c.two_in(0, 59)?;
    let second_at = c.pos;
    let second = if c.peek_digit() {
        Some(c.two_in(0, 59)?)
    } else {
        None
    };
    let zone_at = c.pos;
    let zone = match c.peek() {
        Some(b'Z') => {
            c.pos += 1;
            Zone::Utc
        }
        Some(b) if is_sign(b) => offset_zone(&mut c, false)?,
        _ => return Err(bad(c.pos)),
    };
    c.done()?;
    if mode.is_der() {
        if second.is_none() {
            return Err(der(second_at));
        }
        if zone != Zone::Utc {
            return Err(der(zone_at));
        }
    }
    let year = if yy >= pivot {
        1900 + u16::from(yy)
    } else {
        2000 + u16::from(yy)
    };
    Ok(Fields {
        year,
        month,
        day,
        hour,
        minute: Some(minute),
        second,
        fraction: &[],
        comma: false,
        zone,
    })
}

/// Parse `GeneralizedTime` contents. X.680 §46.2, §46.3 (BER); in DER also X.690 §11.7.
///
/// `YYYYMMDDhh`, optional `mm`, optional `ss`, an optional fraction (`,` or `.` and one or more digits, of the
/// last unit present, any number of digits), then no zone (local time), `Z`, or a sign and `hh` or `hhmm`.
/// Hour 24 is excluded (X.680 §46.2 a): midnight at the end of a day is not a value). POLICY (`[unread]` ISO
/// 8601): second 00 to 59 (no leap second), time difference hours 00 to 23 and minutes 00 to 59, a decimal sign
/// must be followed by a digit.
///
/// DER (X.690 §11.7): the seconds present (§11.7.2), the decimal sign `.` (§11.7.4), no trailing zero in the
/// fraction and no fraction at all if it is zero (§11.7.3), and the zone `Z` (§11.7.1), all as
/// [`ErrorClass::TimeFormat`] at the offending octet. Notably `19920722132100.3Z` (X.690 §11.7.5) is valid DER:
/// this function does NOT apply the RFC 5280 profile, see [`parse_rfc5280_generalized_time`].
///
/// # Errors
/// [`ErrorClass::BadContents`] for a grammar or range violation, [`ErrorClass::TimeFormat`] for DER.
pub fn parse_generalized_time(content: &[u8], mode: Mode) -> Result<Fields<'_>, Error> {
    let mut c = Cursor { s: content, pos: 0 };
    let y1 = u16::from(c.two()?);
    let y2 = u16::from(c.two()?);
    let year = y1 * 100 + y2;
    let month = c.two_in(1, 12)?;
    let day = c.two_in(1, 31)?;
    let hour = c.two_in(0, 23)?;
    let minute = if c.peek_digit() {
        Some(c.two_in(0, 59)?)
    } else {
        None
    };
    let second_at = c.pos;
    let second = if minute.is_some() && c.peek_digit() {
        Some(c.two_in(0, 59)?)
    } else {
        None
    };
    let sign_at = c.pos;
    let mut comma = false;
    let mut fraction: &[u8] = &[];
    if let Some(b @ (b'.' | b',')) = c.peek() {
        comma = b == b',';
        c.pos += 1;
        let from = c.pos;
        if !c.peek_digit() {
            return Err(bad(from));
        }
        while c.peek_digit() {
            c.pos += 1;
        }
        fraction = &content[from..c.pos];
    }
    let zone_at = c.pos;
    let zone = match c.peek() {
        None => Zone::Local,
        Some(b'Z') => {
            c.pos += 1;
            Zone::Utc
        }
        Some(b) if is_sign(b) => offset_zone(&mut c, true)?,
        _ => return Err(bad(c.pos)),
    };
    c.done()?;
    if mode.is_der() {
        if second.is_none() {
            return Err(der(second_at));
        }
        if comma {
            return Err(der(sign_at));
        }
        if fraction.last() == Some(&b'0') {
            return Err(der(zone_at - 1));
        }
        if zone != Zone::Utc {
            return Err(der(zone_at));
        }
    }
    Ok(Fields {
        year,
        month,
        day,
        hour,
        minute,
        second,
        fraction,
        comma,
        zone,
    })
}

/// The RFC 5280 profile of `UTCTime`: `YYMMDDHHMMSSZ` with the century pivot of [`RFC5280_PIVOT`].
///
/// RFC 5280 §4.1.2.5.1: values are in Zulu, include seconds, and YY of 50 or more is 19YY. This is the same set
/// as DER (X.690 §11.8), with the pivot fixed.
///
/// # Errors
/// As [`parse_utc_time`] in DER mode.
pub fn parse_rfc5280_utc_time(content: &[u8]) -> Result<Fields<'_>, Error> {
    parse_utc_time(content, Mode::Der, RFC5280_PIVOT)
}

/// The RFC 5280 profile of `GeneralizedTime`: `YYYYMMDDHHMMSSZ`, Zulu, seconds present, NO fractional seconds.
///
/// RFC 5280 §4.1.2.5.2. This is NARROWER than X.690 DER (§11.7.3 allows a fraction without trailing zeros, as in
/// the standard's own valid example `19920722132100.3Z`). A fraction is refused as [`ErrorClass::TimeFormat`] at
/// the decimal sign. Use [`parse_generalized_time`] for X.690 DER.
///
/// # Errors
/// As [`parse_generalized_time`] in DER mode, and a fraction.
pub fn parse_rfc5280_generalized_time(content: &[u8]) -> Result<Fields<'_>, Error> {
    let f = parse_generalized_time(content, Mode::Der)?;
    if f.fraction.is_empty() {
        Ok(f)
    } else {
        Err(der(14))
    }
}

/// A DATE value. X.680 §38.4.1, X.690 §8.26.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    /// 1582 to 9999 (`Year=Basic`).
    pub year: u16,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
}

/// A `TIME-OF-DAY` value. X.680 §38.4.2, X.690 §8.26.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeOfDay {
    /// 0 to 24; 24 only as `24:00:00` (midnight at the end of the day, X.680 §38.4.2 NOTE).
    pub hour: u8,
    /// 0 to 59.
    pub minute: u8,
    /// 0 to 59 (POLICY, `[unread]` ISO 8601).
    pub second: u8,
}

/// A `DATE-TIME` value. X.680 §38.4.3, X.690 §8.26.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    /// The date part.
    pub date: Date,
    /// The time part.
    pub time: TimeOfDay,
}

fn date_at(c: &mut Cursor<'_>) -> Result<Date, Error> {
    let at = c.pos;
    let y1 = u16::from(c.two()?);
    let y2 = u16::from(c.two()?);
    let year = y1 * 100 + y2;
    // Year=Basic: 1582 to 9999. From the notes (M15) and X.680 §38.4.1; Table 6 itself was not re-read.
    if year < 1582 {
        return Err(bad(at));
    }
    let month = c.two_in(1, 12)?;
    let day = c.two_in(1, 31)?;
    Ok(Date { year, month, day })
}

fn time_at(c: &mut Cursor<'_>) -> Result<TimeOfDay, Error> {
    let at = c.pos;
    let hour = c.two_in(0, 24)?;
    let minute = c.two_in(0, 59)?;
    let second = c.two_in(0, 59)?;
    if hour == 24 && (minute != 0 || second != 0) {
        return Err(bad(at));
    }
    Ok(TimeOfDay {
        hour,
        minute,
        second,
    })
}

/// Parse DATE contents: `YYYYMMDD`, the value notation `YYYY-MM-DD` with the hyphens removed. X.690 §8.26.2.2.
///
/// There is no difference between BER and DER for this type (X.690 §11.9.1 has nothing for it).
///
/// # Errors
/// [`ErrorClass::BadContents`].
pub fn parse_date(content: &[u8]) -> Result<Date, Error> {
    let mut c = Cursor { s: content, pos: 0 };
    let d = date_at(&mut c)?;
    c.done()?;
    Ok(d)
}

/// Parse `TIME-OF-DAY` contents: `hhmmss`, the notation `hh:mm:ss` with the colons removed. X.690 §8.26.3.2.
///
/// # Errors
/// [`ErrorClass::BadContents`].
pub fn parse_time_of_day(content: &[u8]) -> Result<TimeOfDay, Error> {
    let mut c = Cursor { s: content, pos: 0 };
    let t = time_at(&mut c)?;
    c.done()?;
    Ok(t)
}

/// Parse `DATE-TIME` contents: `YYYYMMDDhhmmss`, the notation with the hyphens, colons and `T` removed. X.690
/// §8.26.4.2.
///
/// # Errors
/// [`ErrorClass::BadContents`].
pub fn parse_date_time(content: &[u8]) -> Result<DateTime, Error> {
    let mut c = Cursor { s: content, pos: 0 };
    let date = date_at(&mut c)?;
    let time = time_at(&mut c)?;
    c.done()?;
    Ok(DateTime { date, time })
}

/// Validate DURATION contents: the value notation with the initial `P` removed. X.690 §8.26.5.2, X.680 §38.3.6.
///
/// Grammar (X.680 §38.3.6.1 to §38.3.6.14, after the `P`): either one week designation (`nW`), or year, month,
/// day designations in that order, optionally followed by `T` and hour, minute, second designations in that
/// order, or `T` and the time designations alone. At least one designation, and at least one after a `T`. A
/// designation is digits, optionally a decimal sign (`.` or `,`) and one or more digits, then its letter. The
/// integral part has no leading zero unless it is the single digit 0 (§38.3.6.11); a designation with a
/// fractional part is the last (§38.3.6.13).
///
/// DER (X.690 §11.9.1): a comma as decimal sign (a), and any zero component other than the last one present (d)
/// are [`ErrorClass::TimeFormat`]. The rule (b) and (c) do not apply to a bare duration. Accuracy is never
/// normalised (§38.3.6.14): `29M` and `29M0D` are different values and both are canonical.
///
/// # Errors
/// [`ErrorClass::BadContents`], [`ErrorClass::TimeFormat`] (the first violation of the grammar is returned in
/// preference to a DER violation).
pub fn validate_duration(content: &[u8], mode: Mode) -> Result<(), Error> {
    let mut c = Cursor { s: content, pos: 0 };
    if content.is_empty() {
        return Err(bad(0));
    }
    let mut in_time = false;
    let mut last_rank = 0u8;
    let mut prev_fraction = false;
    let mut prev_zero_at: Option<usize> = None;
    let mut der_err: Option<Error> = None;
    let mut count = 0usize;
    let mut week = false;
    let mut t_at: Option<usize> = None;
    while let Some(b) = c.peek() {
        if week {
            return Err(bad(c.pos));
        }
        if b == b'T' {
            if in_time || prev_fraction {
                return Err(bad(c.pos));
            }
            in_time = true;
            t_at = Some(c.pos);
            c.pos += 1;
            if !c.peek_digit() {
                return Err(bad(c.pos));
            }
            continue;
        }
        let start = c.pos;
        if prev_fraction {
            return Err(bad(start));
        }
        if let (Some(z), None) = (prev_zero_at.take(), der_err) {
            der_err = Some(der(z));
        }
        // Integral part.
        let int_from = c.pos;
        while c.peek_digit() {
            c.pos += 1;
        }
        if c.pos == int_from {
            return Err(bad(c.pos));
        }
        let int = &content[int_from..c.pos];
        if int.len() > 1 && int[0] == b'0' {
            return Err(bad(int_from));
        }
        let mut zero = int == b"0";
        let mut has_fraction = false;
        if let Some(sep @ (b'.' | b',')) = c.peek() {
            if sep == b',' && der_err.is_none() {
                der_err = Some(der(c.pos));
            }
            c.pos += 1;
            let f_from = c.pos;
            while c.peek_digit() {
                c.pos += 1;
            }
            if c.pos == f_from {
                return Err(bad(c.pos));
            }
            zero &= content[f_from..c.pos].iter().all(|&d| d == b'0');
            has_fraction = true;
        }
        let rank = match (c.peek(), in_time) {
            (Some(b'Y'), false) => 1,
            (Some(b'M'), false) => 2,
            (Some(b'D'), false) => 3,
            (Some(b'H'), true) => 4,
            (Some(b'M'), true) => 5,
            (Some(b'S'), true) => 6,
            (Some(b'W'), false) if count == 0 && t_at.is_none() => {
                week = true;
                7
            }
            _ => return Err(bad(c.pos)),
        };
        if rank <= last_rank {
            return Err(bad(c.pos));
        }
        last_rank = rank;
        c.pos += 1;
        count += 1;
        prev_fraction = has_fraction;
        if zero {
            prev_zero_at = Some(start);
        }
    }
    if count == 0 {
        return Err(bad(c.pos));
    }
    match (mode.is_der(), der_err) {
        (true, Some(e)) => Err(e),
        _ => Ok(()),
    }
}

/// Validate the contents of a general TIME value, as far as this crate goes. X.690 §8.26.1.2, §11.9.1.
///
/// The contents are the UTF-8 of the value notation with the quotation marks removed, so every octet must be a
/// printable ASCII character (0x20 to 0x7E, the `simplestring` of X.680 §12.16, taken from the notes, not re-read).
/// In DER the canonical transformations that can be seen without parsing the notation are checked: a comma
/// (X.690 §11.9.1 a), and a time difference of a whole number of hours written with minutes, `+hh:00`
/// (§11.9.1 b). The full TIME grammar of X.680 Table 6 and Table 7 (intervals, recurrence, week and ordinal
/// dates, centuries) is NOT implemented, and neither are §11.9.1 c) and d) for intervals: those rows are open.
///
/// # Errors
/// [`ErrorClass::BadContents`], [`ErrorClass::TimeFormat`].
pub fn validate_time(content: &[u8], mode: Mode) -> Result<(), Error> {
    if let Some(i) = content.iter().position(|b| !(0x20..=0x7E).contains(b)) {
        return Err(bad(i));
    }
    if mode.is_der() {
        for (i, &b) in content.iter().enumerate() {
            if b == b',' {
                return Err(der(i));
            }
            if is_sign(b) {
                let r = &content[i + 1..];
                let hhmm = r.len() >= 5
                    && r[..2].iter().all(u8::is_ascii_digit)
                    && r[2] == b':'
                    && &r[3..5] == b"00"
                    && !r.get(5).is_some_and(u8::is_ascii_digit);
                if hhmm {
                    return Err(der(i + 3));
                }
            }
        }
    }
    Ok(())
}
