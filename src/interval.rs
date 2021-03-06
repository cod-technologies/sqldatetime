//! Interval implementation.

use crate::common::{
    HOURS_PER_DAY, MINUTES_PER_HOUR, MONTHS_PER_YEAR, SECONDS_PER_MINUTE, USECONDS_MAX,
    USECONDS_PER_DAY, USECONDS_PER_HOUR, USECONDS_PER_MINUTE, USECONDS_PER_SECOND,
};
use crate::error::{Error, Result};
use crate::format::{LazyFormat, NaiveDateTime};
use crate::interval::Sign::{Negative, Positive};
use crate::{Date, Time};
use crate::{DateTime, Formatter};
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt::Display;
use std::ops::Neg;

const INTERVAL_MAX_YEAR: i32 = 178_000_000;
const INTERVAL_MAX_DAY: i32 = 100_000_000;

pub(crate) const INTERVAL_MAX_MONTH: i32 = INTERVAL_MAX_YEAR * (MONTHS_PER_YEAR as i32);
pub(crate) const INTERVAL_MAX_USECONDS: i64 = INTERVAL_MAX_DAY as i64 * USECONDS_PER_DAY;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Sign {
    Positive = 1,
    Negative = -1,
}

/// `Year-Month Interval` represents the duration of a period of time,
/// has an interval precision that includes a YEAR field or a MONTH field, or both.
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(transparent)]
pub struct IntervalYM(i32);

impl IntervalYM {
    /// The smallest interval that can be represented by `IntervalYM`, i.e. `-178000000-00`.
    pub const MIN: Self = unsafe { IntervalYM::from_ym_unchecked(178000000, 0).negate() };

    /// The largest interval that can be represented by `IntervalYM`, i.e. `178000000-00`.
    pub const MAX: Self = unsafe { IntervalYM::from_ym_unchecked(178000000, 0) };

    /// The zero value of interval, i.e. `00-00`.
    pub const ZERO: Self = IntervalYM(0);

    /// Creates a `IntervalYM` from the given year and month.
    ///
    /// # Safety
    /// This function is unsafe because the values are not checked for validity!
    /// Before using it, check that the values are all correct.
    #[inline]
    pub const unsafe fn from_ym_unchecked(year: u32, month: u32) -> Self {
        IntervalYM((year * MONTHS_PER_YEAR + month) as i32)
    }

    /// Creates a `IntervalYM` from the given months without checking validity.
    ///
    /// # Safety
    /// This function is unsafe because the `months` is not checked for validity!
    /// Before using it, check that the value is correct.
    #[inline(always)]
    pub const unsafe fn from_months_unchecked(months: i32) -> Self {
        IntervalYM(months)
    }

    /// Creates a `IntervalYM` from the given year and month.
    #[inline]
    pub const fn try_from_ym(year: u32, month: u32) -> Result<Self> {
        if year >= INTERVAL_MAX_YEAR as u32 && (year != INTERVAL_MAX_YEAR as u32 || month != 0) {
            return Err(Error::IntervalOutOfRange);
        }

        if month >= MONTHS_PER_YEAR {
            return Err(Error::InvalidMonth);
        }

        Ok(unsafe { IntervalYM::from_ym_unchecked(year, month) })
    }

    /// Creates a `IntervalYM` from the given months.
    #[inline]
    pub const fn try_from_months(months: i32) -> Result<Self> {
        if IntervalYM::is_valid_months(months) {
            Ok(unsafe { IntervalYM::from_months_unchecked(months) })
        } else {
            Err(Error::IntervalOutOfRange)
        }
    }

    /// Checks if the given year and month are valid.
    #[inline]
    pub const fn is_valid_ym(year: u32, month: u32) -> bool {
        if year >= INTERVAL_MAX_YEAR as u32 && (year != INTERVAL_MAX_YEAR as u32 || month != 0) {
            return false;
        }

        if month >= MONTHS_PER_YEAR {
            return false;
        }

        true
    }

    /// Check if the `months` is valid for IntervalYM
    #[inline]
    pub(crate) const fn is_valid_months(months: i32) -> bool {
        months <= INTERVAL_MAX_MONTH && months >= -INTERVAL_MAX_MONTH
    }

    /// Gets the months of `IntervalYM`.
    #[inline(always)]
    pub const fn months(self) -> i32 {
        self.0
    }

    /// Extracts `(sign, year, month)` from the interval.
    #[inline]
    pub const fn extract(self) -> (Sign, u32, u32) {
        if self.0.is_negative() {
            let year = -self.0 as u32 / MONTHS_PER_YEAR;
            (Negative, year, -self.0 as u32 - year * MONTHS_PER_YEAR)
        } else {
            let year = self.0 as u32 / MONTHS_PER_YEAR;
            (Positive, year, self.0 as u32 - year * MONTHS_PER_YEAR)
        }
    }

    /// Formats `IntervalYM` by given format string.
    #[inline]
    pub fn format<S: AsRef<str>>(self, fmt: S) -> Result<impl Display> {
        let fmt = Formatter::try_new(fmt)?;
        Ok(LazyFormat::new(fmt, self))
    }

    /// Parses `IntervalYM` from given string and format.
    #[inline]
    pub fn parse<S1: AsRef<str>, S2: AsRef<str>>(input: S1, fmt: S2) -> Result<Self> {
        let fmt = Formatter::try_new(fmt)?;
        fmt.parse(input)
    }

    #[inline]
    pub(crate) const fn negate(self) -> IntervalYM {
        unsafe { IntervalYM::from_months_unchecked(-self.months()) }
    }

    /// `IntervalYM` adds `IntervalYM`
    #[inline]
    pub const fn add_interval_ym(self, interval: IntervalYM) -> Result<IntervalYM> {
        let result = self.months().checked_add(interval.months());
        match result {
            Some(i) => IntervalYM::try_from_months(i),
            None => Err(Error::IntervalOutOfRange),
        }
    }

    /// `IntervalYM` subtracts `IntervalYM`
    #[inline]
    pub const fn sub_interval_ym(self, interval: IntervalYM) -> Result<IntervalYM> {
        self.add_interval_ym(interval.negate())
    }

    /// `IntervalYM` multiplies `f64`
    #[inline]
    pub fn mul_f64(self, number: f64) -> Result<IntervalYM> {
        let months = self.months() as f64;
        let result = months * number;

        if result.is_infinite() {
            Err(Error::NumericOverflow)
        } else if result.is_nan() {
            Err(Error::InvalidNumber)
        } else {
            IntervalYM::try_from_months(result as i32)
        }
    }

    /// `IntervalYM` divides `f64`
    #[inline]
    pub fn div_f64(self, number: f64) -> Result<IntervalYM> {
        if number == 0.0 {
            return Err(Error::DivideByZero);
        }
        let months = self.months() as f64;
        let result = months / number;

        if result.is_infinite() {
            Err(Error::NumericOverflow)
        } else if result.is_nan() {
            Err(Error::InvalidNumber)
        } else {
            IntervalYM::try_from_months(result as i32)
        }
    }
}

impl From<IntervalYM> for NaiveDateTime {
    #[inline]
    fn from(interval: IntervalYM) -> Self {
        let (sign, year, month) = interval.extract();
        let negative = sign == Negative;
        NaiveDateTime {
            year: year as i32,
            month,
            negative,
            ..NaiveDateTime::new()
        }
    }
}

impl TryFrom<NaiveDateTime> for IntervalYM {
    type Error = Error;

    #[inline]
    fn try_from(dt: NaiveDateTime) -> Result<Self> {
        if dt.negative {
            Ok(-IntervalYM::try_from_ym(-dt.year as u32, dt.month)?)
        } else {
            IntervalYM::try_from_ym(dt.year as u32, dt.month)
        }
    }
}

impl Neg for IntervalYM {
    type Output = IntervalYM;

    #[inline]
    fn neg(self) -> Self::Output {
        self.negate()
    }
}

impl DateTime for IntervalYM {
    #[inline(always)]
    fn year(&self) -> Option<i32> {
        Some(self.months() / MONTHS_PER_YEAR as i32)
    }

    #[inline(always)]
    fn month(&self) -> Option<i32> {
        Some(self.months() % MONTHS_PER_YEAR as i32)
    }

    #[inline(always)]
    fn day(&self) -> Option<i32> {
        None
    }

    #[inline(always)]
    fn hour(&self) -> Option<i32> {
        None
    }

    #[inline(always)]
    fn minute(&self) -> Option<i32> {
        None
    }

    #[inline(always)]
    fn second(&self) -> Option<f64> {
        None
    }

    #[inline(always)]
    fn date(&self) -> Option<Date> {
        None
    }
}

/// `Day-Time Interval` represents the duration of a period of time,
/// has an interval precision that includes DAY, HOUR, MINUTE, SECOND, MICROSECOND.
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct IntervalDT(i64);

impl IntervalDT {
    /// The smallest interval that can be represented by `IntervalDT`, i.e. `-100000000 00:00:00.000000`.
    pub const MIN: Self =
        unsafe { IntervalDT::from_dhms_unchecked(100000000, 0, 0, 0, 0).negate() };

    /// The largest interval that can be represented by `IntervalDT`, i.e. `100000000 00:00:00.000000`.
    pub const MAX: Self = unsafe { IntervalDT::from_dhms_unchecked(100000000, 0, 0, 0, 0) };

    /// The zero value of interval, i.e. `0 00:00:00.000000`.
    pub const ZERO: Self = IntervalDT(0);

    /// Creates a `IntervalDT` from the given day, hour, minute, second and microsecond.
    ///
    /// # Safety
    /// This function is unsafe because the values are not checked for validity!
    /// Before using it, check that the values are all correct.
    #[inline]
    pub const unsafe fn from_dhms_unchecked(
        day: u32,
        hour: u32,
        minute: u32,
        sec: u32,
        usec: u32,
    ) -> Self {
        let time = hour as i64 * USECONDS_PER_HOUR
            + minute as i64 * USECONDS_PER_MINUTE
            + sec as i64 * USECONDS_PER_SECOND
            + usec as i64;
        let us = day as i64 * USECONDS_PER_DAY + time;
        IntervalDT(us)
    }

    /// Creates a `IntervalDT` from the given day, hour, minute, second and microsecond.
    #[inline]
    pub const fn try_from_dhms(
        day: u32,
        hour: u32,
        minute: u32,
        sec: u32,
        usec: u32,
    ) -> Result<Self> {
        if day >= INTERVAL_MAX_DAY as u32
            && (day != INTERVAL_MAX_DAY as u32 || hour != 0 || minute != 0 || sec != 0 || usec != 0)
        {
            return Err(Error::IntervalOutOfRange);
        }

        if hour >= HOURS_PER_DAY {
            return Err(Error::TimeOutOfRange);
        }

        if minute >= MINUTES_PER_HOUR {
            return Err(Error::InvalidMinute);
        }

        if sec >= SECONDS_PER_MINUTE {
            return Err(Error::InvalidSecond);
        }

        if usec > USECONDS_MAX {
            return Err(Error::InvalidFraction);
        }

        Ok(unsafe { IntervalDT::from_dhms_unchecked(day, hour, minute, sec, usec) })
    }

    /// Creates a `IntervalDT` from the given microseconds without checking the validity.
    ///
    /// # Safety
    /// This function is unsafe because the microsecond value is not checked for validity!
    /// Before using it, check that the value is all correct.
    #[inline(always)]
    pub const unsafe fn from_usecs_unchecked(usecs: i64) -> Self {
        IntervalDT(usecs)
    }

    /// Creates a `IntervalDT` from the given microseconds.
    #[inline]
    pub const fn try_from_usecs(usecs: i64) -> Result<Self> {
        if IntervalDT::is_valid_usecs(usecs) {
            Ok(unsafe { IntervalDT::from_usecs_unchecked(usecs) })
        } else {
            Err(Error::IntervalOutOfRange)
        }
    }

    /// Checks if the given day, hour, minute, second and microsecond fields are valid.
    #[inline]
    pub const fn is_valid(day: u32, hour: u32, minute: u32, sec: u32, usec: u32) -> bool {
        if day >= INTERVAL_MAX_DAY as u32
            && (day != INTERVAL_MAX_DAY as u32 || hour != 0 || minute != 0 || sec != 0 || usec != 0)
        {
            return false;
        }

        if hour >= HOURS_PER_DAY {
            return false;
        }

        if minute >= MINUTES_PER_HOUR {
            return false;
        }

        if sec >= SECONDS_PER_MINUTE {
            return false;
        }

        if usec > USECONDS_MAX {
            return false;
        }

        true
    }

    #[inline]
    pub(crate) const fn is_valid_usecs(usecs: i64) -> bool {
        usecs <= INTERVAL_MAX_USECONDS && usecs >= -INTERVAL_MAX_USECONDS
    }

    /// Gets the microseconds of `IntervalDT`.
    #[inline(always)]
    pub const fn usecs(self) -> i64 {
        self.0
    }

    /// Extracts `(sign, day, hour, minute, second, microsecond)` from the interval.
    #[inline]
    pub const fn extract(self) -> (Sign, u32, u32, u32, u32, u32) {
        let (sign, day, mut time) = if self.0.is_negative() {
            let day = -self.0 / USECONDS_PER_DAY;
            (Negative, day, -self.0 - day * USECONDS_PER_DAY)
        } else {
            let day = self.0 / USECONDS_PER_DAY;
            (Positive, day, self.0 - day * USECONDS_PER_DAY)
        };

        let hour = time / USECONDS_PER_HOUR;
        time -= hour * USECONDS_PER_HOUR;

        let minute = time / USECONDS_PER_MINUTE;
        time -= minute * USECONDS_PER_MINUTE;

        let sec = time / USECONDS_PER_SECOND;
        let usec = time - sec * USECONDS_PER_SECOND;

        (
            sign,
            day as u32,
            hour as u32,
            minute as u32,
            sec as u32,
            usec as u32,
        )
    }

    /// Formats `IntervalDT` by given format string.
    #[inline]
    pub fn format<S: AsRef<str>>(self, fmt: S) -> Result<impl Display> {
        let fmt = Formatter::try_new(fmt)?;
        Ok(LazyFormat::new(fmt, self))
    }

    /// Parses `IntervalDT` from given string and format.
    #[inline]
    pub fn parse<S1: AsRef<str>, S2: AsRef<str>>(input: S1, fmt: S2) -> Result<Self> {
        let fmt = Formatter::try_new(fmt)?;
        fmt.parse(input)
    }

    #[inline]
    pub(crate) const fn negate(self) -> IntervalDT {
        unsafe { IntervalDT::from_usecs_unchecked(-self.usecs()) }
    }

    /// `IntervalDT` adds `IntervalDT`
    #[inline]
    pub const fn add_interval_dt(self, interval: IntervalDT) -> Result<IntervalDT> {
        let result = self.usecs().checked_add(interval.usecs());
        match result {
            Some(i) => IntervalDT::try_from_usecs(i),
            None => Err(Error::IntervalOutOfRange),
        }
    }

    /// `IntervalDT` subtracts `IntervalDT`
    #[inline]
    pub const fn sub_interval_dt(self, interval: IntervalDT) -> Result<IntervalDT> {
        self.add_interval_dt(interval.negate())
    }

    /// `IntervalDT` multiplies `f64`
    #[inline]
    pub fn mul_f64(self, number: f64) -> Result<IntervalDT> {
        let usecs = self.usecs() as f64;
        let result = usecs * number;

        if result.is_infinite() {
            Err(Error::NumericOverflow)
        } else if result.is_nan() {
            Err(Error::InvalidNumber)
        } else {
            IntervalDT::try_from_usecs(result as i64)
        }
    }

    /// `IntervalDT` divides `f64`
    #[inline]
    pub fn div_f64(self, number: f64) -> Result<IntervalDT> {
        if number == 0.0 {
            return Err(Error::DivideByZero);
        }
        let usecs = self.usecs() as f64;
        let result = usecs / number;

        if result.is_infinite() {
            Err(Error::NumericOverflow)
        } else if result.is_nan() {
            Err(Error::InvalidNumber)
        } else {
            IntervalDT::try_from_usecs(result as i64)
        }
    }

    /// `IntervalDT` subtracts `Time`
    #[inline]
    pub const fn sub_time(self, time: Time) -> Result<IntervalDT> {
        IntervalDT::try_from_usecs(self.usecs() - time.usecs())
    }
}

impl From<IntervalDT> for NaiveDateTime {
    #[inline]
    fn from(interval: IntervalDT) -> Self {
        let (sign, day, hour, minute, sec, usec) = interval.extract();
        let negative = sign == Sign::Negative;
        NaiveDateTime {
            day,
            hour,
            minute,
            sec,
            usec,
            negative,
            ..NaiveDateTime::new()
        }
    }
}

impl TryFrom<NaiveDateTime> for IntervalDT {
    type Error = Error;

    #[inline]
    fn try_from(dt: NaiveDateTime) -> Result<Self> {
        if dt.negative {
            Ok(IntervalDT::try_from_dhms(dt.day, dt.hour, dt.minute, dt.sec, dt.usec)?.negate())
        } else {
            IntervalDT::try_from_dhms(dt.day, dt.hour, dt.minute, dt.sec, dt.usec)
        }
    }
}

impl From<Time> for IntervalDT {
    #[inline]
    fn from(time: Time) -> Self {
        unsafe { IntervalDT::from_usecs_unchecked(time.usecs()) }
    }
}

impl PartialEq<Time> for IntervalDT {
    #[inline]
    fn eq(&self, other: &Time) -> bool {
        self.usecs() == other.usecs()
    }
}

impl PartialOrd<Time> for IntervalDT {
    #[inline]
    fn partial_cmp(&self, other: &Time) -> Option<Ordering> {
        Some(self.usecs().cmp(&other.usecs()))
    }
}

impl Neg for IntervalDT {
    type Output = IntervalDT;

    #[inline]
    fn neg(self) -> Self::Output {
        self.negate()
    }
}

impl DateTime for IntervalDT {
    #[inline(always)]
    fn year(&self) -> Option<i32> {
        None
    }

    #[inline(always)]
    fn month(&self) -> Option<i32> {
        None
    }

    #[inline(always)]
    fn day(&self) -> Option<i32> {
        Some((self.usecs() / USECONDS_PER_DAY) as i32)
    }

    #[inline(always)]
    fn hour(&self) -> Option<i32> {
        let remain_time = self.usecs() % USECONDS_PER_DAY;
        Some((remain_time / USECONDS_PER_HOUR) as i32)
    }

    #[inline(always)]
    fn minute(&self) -> Option<i32> {
        let remain_time = self.usecs() % USECONDS_PER_HOUR;
        Some((remain_time / USECONDS_PER_MINUTE) as i32)
    }

    #[inline]
    fn second(&self) -> Option<f64> {
        let remain_time = self.usecs() % USECONDS_PER_MINUTE;
        Some(remain_time as f64 / USECONDS_PER_SECOND as f64)
    }

    #[inline(always)]
    fn date(&self) -> Option<Date> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_ym() {
        assert_eq!(IntervalYM::ZERO, IntervalYM::try_from_ym(0, 0).unwrap());
        assert_eq!(
            IntervalYM::MIN,
            -IntervalYM::try_from_ym(178000000, 0).unwrap()
        );
        assert_eq!(
            IntervalYM::MAX,
            IntervalYM::try_from_ym(178000000, 0).unwrap()
        );

        let interval = IntervalYM::try_from_ym(0, 0).unwrap();
        assert_eq!(interval.months(), 0);
        assert_eq!(interval.extract(), (Positive, 0, 0));

        let interval = IntervalYM::try_from_ym(178000000, 0).unwrap();
        assert_eq!(interval.extract(), (Positive, 178000000, 0));
        let fmt = format!("{}", interval.format("yyyy-mm").unwrap());
        assert_eq!(fmt, "+178000000-00");
        let interval2 = IntervalYM::parse("178000000-00", "yyyy-mm").unwrap();
        assert_eq!(interval2, interval);

        let interval = -IntervalYM::try_from_ym(1, 0).unwrap();
        let fmt = format!("{}", interval.format("yy-mm").unwrap());
        assert_eq!(fmt, "-01-00");

        let interval = IntervalYM::try_from_ym(123, 2).unwrap();
        let fmt = format!("{}", interval.format("yy-mm").unwrap());
        assert_eq!(fmt, "+123-02");

        let interval = -IntervalYM::try_from_ym(178000000, 0).unwrap();
        assert_eq!(interval.extract(), (Negative, 178000000, 0));
        let interval = IntervalYM::try_from_ym(178000000, 0).unwrap().negate();
        assert_eq!(interval.extract(), (Negative, 178000000, 0));
        let fmt = format!("{}", interval.format("yyyy-mm").unwrap());
        assert_eq!(fmt, "-178000000-00");

        let fmt = format!("{}", interval.format("yy-mm").unwrap());
        assert_eq!(fmt, "-178000000-00");

        let interval2 = IntervalYM::parse("-178000000-00", "yyyy-mm").unwrap();
        assert_eq!(interval2, interval);

        let interval2 = IntervalYM::parse("+178000000-00", "yyyy-mm").unwrap();
        assert_eq!(interval2, -interval);

        let interval = IntervalYM::try_from_ym(177999999, 11).unwrap();
        assert_eq!(interval.extract(), (Positive, 177999999, 11));

        let interval = -IntervalYM::try_from_ym(177999999, 11).unwrap();
        assert_eq!(interval.extract(), (Negative, 177999999, 11));

        let interval = IntervalYM::try_from_months(0).unwrap();
        assert_eq!(interval.extract(), (Positive, 0, 0));

        let interval = IntervalYM::try_from_months(-11).unwrap();
        assert_eq!(interval.extract(), (Negative, 0, 11));
        let fmt = format!("{}", interval.format("yyyy-mm").unwrap());
        assert_eq!(fmt, "-0000-11");

        let interval2 = IntervalYM::parse("-0000-11", "yyyy-mm").unwrap();
        assert_eq!(interval, interval2);
        let interval2 = IntervalYM::parse("-0000 - 11", "yyyy - mm").unwrap();
        assert_eq!(interval, interval2);
        let interval2 = IntervalYM::parse("       -0000 - 11       ", "yyyy - mm").unwrap();
        assert_eq!(interval, interval2);
        let interval2 = IntervalYM::parse("       -0000 - 11       ", "    yyyy - mm    ").unwrap();
        assert_eq!(interval, interval2);
        let interval2 = IntervalYM::parse("-0000-11", "yyyy - mm").unwrap();
        assert_eq!(interval, interval2);

        let interval = IntervalYM::try_from_months(11).unwrap();
        let interval2 = IntervalYM::parse("0000-11", "yyyy-mm").unwrap();
        assert_eq!(interval, interval2);

        let interval = IntervalYM::try_from_ym(12345, 1).unwrap();
        let interval2 = IntervalYM::parse("12345-1", "yyyy-mm").unwrap();
        assert_eq!(interval, interval2);

        let interval = IntervalYM::try_from_ym(12345, 1).unwrap();
        let interval2 = IntervalYM::parse("12345-1", "yy-mm").unwrap();
        assert_eq!(interval, interval2);

        let interval = IntervalYM::try_from_ym(1, 1).unwrap();
        let interval2 = IntervalYM::parse("1-1", "yy-mm").unwrap();
        assert_eq!(interval, interval2);

        // Tests when fmt doesn't have month, returns the default month `0` in IntervalYM
        let res = IntervalYM::parse("2022", "YYYY").unwrap();
        assert_eq!(res.month().unwrap(), 0);

        // Invalid
        assert!(IntervalYM::parse("178000000-1", "yyyy-mm").is_err());
        assert!(IntervalYM::parse("178000001-0", "yyyy-mm").is_err());
        assert!(IntervalYM::parse("-178000001-0", "yyyy-mm").is_err());
        assert!(IntervalYM::parse("0-13", "yyyy-mm").is_err());
        assert!(IntervalYM::parse("-178000000-1", "yyyy-mm").is_err());
        assert!(IntervalYM::parse("-178000001-0", "yyyy-mm").is_err());
        assert!(IntervalYM::parse("11", "dd").is_err());
        assert!(IntervalYM::parse("11", "hh24").is_err());
        assert!(IntervalYM::parse("11", "mi").is_err());
        assert!(IntervalYM::parse("11", "ss").is_err());

        assert_eq!(
            IntervalYM::parse("xxxx", "yy-mm").err().unwrap(),
            Error::ParseError("the interval is invalid".to_string())
        )
        // todo invalid fields
    }

    #[test]
    fn test_interval_dt() {
        assert_eq!(
            IntervalDT::ZERO,
            IntervalDT::try_from_dhms(0, 0, 0, 0, 0).unwrap()
        );
        assert_eq!(
            IntervalDT::MIN,
            -IntervalDT::try_from_dhms(100_000_000, 0, 0, 0, 0).unwrap()
        );
        assert_eq!(
            IntervalDT::MAX,
            IntervalDT::try_from_dhms(100_000_000, 0, 0, 0, 0).unwrap()
        );

        let time = Time::try_from_hms(0, 0, 0, 0).unwrap();
        let interval = IntervalDT::from(time);
        assert_eq!(IntervalDT::ZERO, interval);

        let time = Time::try_from_hms(1, 2, 3, 4).unwrap();
        let interval = IntervalDT::from(time);
        assert_eq!(IntervalDT::try_from_dhms(0, 1, 2, 3, 4).unwrap(), interval);

        let time = Time::try_from_hms(23, 59, 59, 999999).unwrap();
        let interval = IntervalDT::from(time);
        assert_eq!(
            IntervalDT::try_from_dhms(0, 23, 59, 59, 999999).unwrap(),
            interval
        );

        let interval = IntervalDT::try_from_dhms(0, 0, 0, 0, 0).unwrap();
        assert_eq!(interval.usecs(), 0);
        assert_eq!(interval.extract(), (Positive, 0, 0, 0, 0, 0));
        let fmt = format!("{}", interval.format("DD HH24:MI:SS").unwrap());
        assert_eq!(fmt, "+00 00:00:00");

        let interval = IntervalDT::try_from_dhms(100000000, 0, 0, 0, 0).unwrap();
        assert_eq!(interval.extract(), (Positive, 100000000, 0, 0, 0, 0));
        let fmt = format!("{}", interval.format("DD HH24:MI:SS").unwrap());
        assert_eq!(fmt, "+100000000 00:00:00");
        let interval2 = IntervalDT::parse("100000000 00:00:00", "DD HH24:MI:SS").unwrap();
        assert_eq!(interval2, interval);

        let interval2 = IntervalDT::parse("+100000000 00:00:00", "DD HH24:MI:SS").unwrap();
        assert_eq!(interval2, interval);

        let interval = -IntervalDT::try_from_dhms(100000000, 0, 0, 0, 0).unwrap();
        assert_eq!(interval.extract(), (Negative, 100000000, 0, 0, 0, 0));

        let interval = IntervalDT::try_from_dhms(100000000, 0, 0, 0, 0)
            .unwrap()
            .negate();
        assert_eq!(interval.extract(), (Negative, 100000000, 0, 0, 0, 0));

        let interval = IntervalDT::try_from_dhms(99999999, 23, 59, 59, 999999).unwrap();
        assert_eq!(interval.extract(), (Positive, 99999999, 23, 59, 59, 999999));

        let interval = -IntervalDT::try_from_dhms(99999999, 23, 59, 59, 999999).unwrap();
        assert_eq!(interval.extract(), (Negative, 99999999, 23, 59, 59, 999999));
        let fmt = format!("{}", interval.format("DD HH24:MI:SS.FF6").unwrap());
        assert_eq!(fmt, "-99999999 23:59:59.999999");

        let interval = IntervalDT::try_from_usecs(-11).unwrap();
        let interval2 = IntervalDT::parse("-0 00:00:00.000011", "DD HH24:MI:SS.FF6").unwrap();
        assert_eq!(interval, interval2);

        let interval = IntervalDT::try_from_usecs(11).unwrap();
        let interval2 = IntervalDT::parse("0 00:00:00.000011", "DD HH24:MI:SS.FF6").unwrap();
        assert_eq!(interval, interval2);

        let interval = IntervalDT::try_from_usecs(-11).unwrap();
        let interval2 = IntervalDT::parse("-0 00:00:00.000011", "DD HH24:MI:SS.FF").unwrap();
        assert_eq!(interval, interval2);

        let interval = IntervalDT::try_from_usecs(-12).unwrap();
        let interval2 = IntervalDT::parse("-0 00:00:00.000011567", "DD HH24:MI:SS.FF").unwrap();
        assert_eq!(interval, interval2);

        let interval = IntervalDT::try_from_dhms(12, 4, 5, 6, 0).unwrap().negate();
        let interval2 = IntervalDT::parse("-12 4:5:6", "DD HH24:MI:SS").unwrap();
        assert_eq!(interval, interval2);

        // Invalid
        assert!(IntervalDT::parse("100000000 02:00:00.0", "DD HH24:MI:SS.FF").is_err());
        assert!(IntervalDT::parse("0 24:00:00:00.0", "DD HH24:MI:SS.FF").is_err());
        assert!(IntervalDT::parse("100000001 00:00:00.0", "DD HH24:MI:SS.FF").is_err());
        assert!(IntervalDT::parse("-100000001 00:00:00.0", "DD HH24:MI:SS.FF").is_err());
        assert!(IntervalDT::parse("-100000000 02:00:00.0", "DD HH24:MI:SS.FF").is_err());

        assert!(IntervalDT::parse("-100000 02:00", "DD HH24:MI:SS.FF").is_err());
        assert!(IntervalDT::parse("-100000 02", "DD HH24:MI:SS.FF").is_err());
        assert!(IntervalDT::parse("-100000 ", "DD HH24:MI:SS.FF").is_err());

        assert!(IntervalDT::parse("1919", "yyyy").is_err());
        assert!(IntervalDT::parse("19", "mm").is_err());
    }

    #[test]
    fn test_interval_negate() {
        assert_eq!(
            -IntervalDT::try_from_dhms(1, 2, 3, 4, 5).unwrap(),
            IntervalDT::try_from_usecs(-93784000005).unwrap()
        );
        assert_eq!(
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5).unwrap().negate(),
            IntervalDT::try_from_usecs(-93784000005).unwrap()
        );
        assert_eq!(
            -IntervalDT::try_from_dhms(0, 0, 0, 0, 0).unwrap(),
            IntervalDT::try_from_dhms(0, 0, 0, 0, 0).unwrap()
        );
        assert_eq!(
            IntervalDT::try_from_dhms(0, 0, 0, 0, 0).unwrap().negate(),
            IntervalDT::try_from_dhms(0, 0, 0, 0, 0).unwrap()
        );
        assert_eq!(
            -IntervalDT::try_from_dhms(1, 2, 3, 4, 5).unwrap().negate(),
            IntervalDT::try_from_usecs(93784000005).unwrap()
        );
        assert_eq!(
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5)
                .unwrap()
                .negate()
                .negate(),
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5).unwrap()
        );
        assert_eq!(
            -IntervalDT::try_from_dhms(INTERVAL_MAX_DAY as u32, 0, 0, 0, 0).unwrap(),
            IntervalDT::try_from_usecs(-8640000000000000000).unwrap()
        );
        assert_eq!(
            IntervalDT::try_from_dhms(INTERVAL_MAX_DAY as u32, 0, 0, 0, 0)
                .unwrap()
                .negate(),
            IntervalDT::try_from_usecs(-8640000000000000000).unwrap()
        );
        assert_eq!(
            -IntervalDT::try_from_dhms(INTERVAL_MAX_DAY as u32, 0, 0, 0, 0).unwrap(),
            IntervalDT::try_from_dhms(INTERVAL_MAX_DAY as u32, 0, 0, 0, 0)
                .unwrap()
                .negate()
        );

        assert_eq!(
            -IntervalYM::try_from_ym(1, 2).unwrap(),
            IntervalYM::try_from_months(-14).unwrap()
        );
        assert_eq!(
            IntervalYM::try_from_ym(1, 2).unwrap().negate(),
            IntervalYM::try_from_months(-14).unwrap()
        );
        assert_eq!(
            -IntervalYM::try_from_ym(0, 0).unwrap(),
            IntervalYM::try_from_ym(0, 0).unwrap()
        );
        assert_eq!(
            IntervalYM::try_from_ym(0, 0).unwrap().negate(),
            IntervalYM::try_from_ym(0, 0).unwrap()
        );
        assert_eq!(
            -IntervalYM::try_from_ym(1, 2).unwrap().negate(),
            IntervalYM::try_from_ym(1, 2).unwrap()
        );
        assert_eq!(
            IntervalYM::try_from_ym(1, 2).unwrap().negate().negate(),
            IntervalYM::try_from_ym(1, 2).unwrap()
        );
        assert_eq!(
            -IntervalYM::try_from_ym(INTERVAL_MAX_YEAR as u32, 0).unwrap(),
            IntervalYM::try_from_months(-2136000000).unwrap()
        );
        assert_eq!(
            IntervalYM::try_from_ym(INTERVAL_MAX_YEAR as u32, 0)
                .unwrap()
                .negate(),
            IntervalYM::try_from_months(-2136000000).unwrap()
        );
        assert_eq!(
            -IntervalYM::try_from_ym(INTERVAL_MAX_YEAR as u32, 0)
                .unwrap()
                .negate(),
            IntervalYM::try_from_months(2136000000).unwrap()
        );
        assert_eq!(
            IntervalYM::try_from_ym(INTERVAL_MAX_YEAR as u32, 0)
                .unwrap()
                .negate()
                .negate(),
            IntervalYM::try_from_months(2136000000).unwrap()
        );
    }

    #[test]
    fn test_interval_ym_add_sub_interval_ym() {
        assert!(IntervalYM::try_from_ym(178000000, 0)
            .unwrap()
            .add_interval_ym(IntervalYM::try_from_ym(0, 1).unwrap())
            .is_err());

        assert!(IntervalYM::try_from_ym(178000000, 0)
            .unwrap()
            .sub_interval_ym(-IntervalYM::try_from_ym(0, 1).unwrap())
            .is_err());

        assert!(IntervalYM::try_from_ym(178000000, 0)
            .unwrap()
            .negate()
            .sub_interval_ym(IntervalYM::try_from_ym(0, 1).unwrap())
            .is_err());

        assert!((-IntervalYM::try_from_ym(178000000, 0).unwrap())
            .add_interval_ym(-IntervalYM::try_from_ym(0, 1).unwrap())
            .is_err());

        assert_eq!(
            IntervalYM::try_from_ym(123456, 5)
                .unwrap()
                .add_interval_ym(IntervalYM::try_from_ym(123, 7).unwrap())
                .unwrap(),
            IntervalYM::try_from_ym(123580, 0).unwrap()
        );

        assert_eq!(
            IntervalYM::try_from_ym(123456, 5)
                .unwrap()
                .sub_interval_ym(IntervalYM::try_from_ym(123, 7).unwrap())
                .unwrap(),
            IntervalYM::try_from_ym(123332, 10).unwrap()
        );
    }

    #[test]
    fn test_interval_dt_add_sub_interval_dt() {
        assert!(IntervalDT::try_from_dhms(100000000, 0, 0, 0, 0)
            .unwrap()
            .add_interval_dt(IntervalDT::try_from_dhms(0, 0, 0, 0, 1).unwrap())
            .is_err());

        assert!(IntervalDT::try_from_dhms(100000000, 0, 0, 0, 0)
            .unwrap()
            .sub_interval_dt(-IntervalDT::try_from_dhms(0, 0, 0, 0, 1).unwrap())
            .is_err());

        assert!(IntervalDT::try_from_dhms(100000000, 0, 0, 0, 0)
            .unwrap()
            .negate()
            .sub_interval_dt(IntervalDT::try_from_dhms(0, 0, 0, 0, 1).unwrap())
            .is_err());

        assert!(IntervalDT::try_from_dhms(100000000, 0, 0, 0, 0)
            .unwrap()
            .negate()
            .add_interval_dt(-IntervalDT::try_from_dhms(0, 0, 0, 0, 1).unwrap())
            .is_err());

        assert_eq!(
            IntervalDT::try_from_dhms(23456789, 1, 2, 3, 4)
                .unwrap()
                .add_interval_dt(IntervalDT::try_from_dhms(123, 1, 2, 3, 4).unwrap())
                .unwrap(),
            IntervalDT::try_from_dhms(23456912, 2, 4, 6, 8).unwrap()
        );

        assert_eq!(
            IntervalDT::try_from_dhms(23456789, 1, 2, 3, 4)
                .unwrap()
                .sub_interval_dt(IntervalDT::try_from_dhms(123, 1, 2, 3, 4).unwrap())
                .unwrap(),
            IntervalDT::try_from_dhms(23456666, 0, 0, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_interval_mul_div() {
        // Normal
        assert_eq!(
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5)
                .unwrap()
                .mul_f64(5.0)
                .unwrap(),
            IntervalDT::try_from_dhms(5, 10, 15, 20, 25).unwrap()
        );

        assert_eq!(
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5)
                .unwrap()
                .mul_f64(-5.2)
                .unwrap(),
            -IntervalDT::try_from_dhms(5, 15, 27, 56, 800026).unwrap()
        );

        assert_eq!(
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5)
                .unwrap()
                .div_f64(-5.2)
                .unwrap(),
            -IntervalDT::try_from_dhms(0, 5, 0, 35, 384616).unwrap()
        );

        assert_eq!(
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5)
                .unwrap()
                .div_f64(-5.0)
                .unwrap(),
            -IntervalDT::try_from_dhms(0, 5, 12, 36, 800001).unwrap()
        );

        assert_eq!(
            IntervalDT::try_from_dhms(99999, 2, 3, 4, 5)
                .unwrap()
                .div_f64(f64::INFINITY)
                .unwrap(),
            -IntervalDT::try_from_dhms(0, 0, 0, 0, 0).unwrap()
        );

        // Round
        assert_eq!(
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5)
                .unwrap()
                .div_f64(-5.1)
                .unwrap(),
            -IntervalDT::try_from_dhms(0, 5, 6, 29, 19608).unwrap()
        );

        assert_eq!(
            IntervalDT::try_from_dhms(1, 2, 3, 4, 5)
                .unwrap()
                .mul_f64(-5.57)
                .unwrap(),
            -IntervalDT::try_from_dhms(6, 1, 6, 16, 880027).unwrap()
        );

        // Out of range
        assert!(IntervalDT::try_from_dhms(99999, 2, 3, 4, 5)
            .unwrap()
            .mul_f64(-1234567890.6)
            .is_err());

        assert!(IntervalDT::try_from_dhms(99999, 2, 3, 4, 5)
            .unwrap()
            .div_f64(-0.000000000001)
            .is_err());

        assert!(IntervalDT::try_from_dhms(99999, 2, 3, 4, 5)
            .unwrap()
            .mul_f64(f64::NEG_INFINITY)
            .is_err());

        assert!(IntervalDT::try_from_dhms(99999, 2, 3, 4, 5)
            .unwrap()
            .div_f64(f64::NAN)
            .is_err());

        assert!(IntervalDT::try_from_dhms(99999, 2, 3, 4, 5)
            .unwrap()
            .mul_f64(f64::NAN)
            .is_err());

        // Divide by zero
        assert!(IntervalDT::try_from_dhms(99999, 2, 3, 4, 5)
            .unwrap()
            .div_f64(0.0)
            .is_err());

        // Year to month
        assert_eq!(
            IntervalYM::try_from_ym(1, 2).unwrap().mul_f64(5.0).unwrap(),
            IntervalYM::try_from_ym(5, 10).unwrap()
        );

        assert_eq!(
            IntervalYM::try_from_ym(1, 2)
                .unwrap()
                .mul_f64(-5.3)
                .unwrap(),
            -IntervalYM::try_from_ym(6, 2).unwrap()
        );

        assert_eq!(
            IntervalYM::try_from_ym(1, 2)
                .unwrap()
                .mul_f64(-5.2)
                .unwrap(),
            -IntervalYM::try_from_ym(6, 0).unwrap()
        );

        assert_eq!(
            IntervalYM::try_from_ym(1, 2)
                .unwrap()
                .div_f64(-5.2)
                .unwrap(),
            -IntervalYM::try_from_ym(0, 2).unwrap()
        );

        assert_eq!(
            IntervalYM::try_from_ym(1, 2)
                .unwrap()
                .div_f64(-4.7)
                .unwrap(),
            -IntervalYM::try_from_ym(0, 2).unwrap()
        );

        assert_eq!(
            IntervalYM::try_from_ym(1, 2)
                .unwrap()
                .div_f64(f64::INFINITY)
                .unwrap(),
            -IntervalYM::try_from_ym(0, 0).unwrap()
        );

        // Out of range
        assert!(IntervalYM::try_from_ym(500000, 2)
            .unwrap()
            .mul_f64(123456789.123)
            .is_err());

        assert!(IntervalYM::try_from_ym(500000, 2)
            .unwrap()
            .mul_f64(f64::INFINITY)
            .is_err());

        assert!(IntervalYM::try_from_ym(500000, 2)
            .unwrap()
            .mul_f64(f64::NEG_INFINITY)
            .is_err());

        assert!(IntervalYM::try_from_ym(500000, 2)
            .unwrap()
            .mul_f64(f64::NAN)
            .is_err());

        assert!(IntervalYM::try_from_ym(500000, 2)
            .unwrap()
            .div_f64(f64::NAN)
            .is_err());

        // Divide by zero
        assert!(IntervalYM::try_from_ym(500000, 2)
            .unwrap()
            .div_f64(0.0)
            .is_err());
    }

    #[test]
    fn test_interval_dt_sub_time() {
        // Out of range
        assert!(
            IntervalDT::try_from_dhms(INTERVAL_MAX_DAY as u32, 0, 0, 0, 0)
                .unwrap()
                .negate()
                .sub_time(Time::try_from_hms(1, 2, 3, 4).unwrap())
                .is_err()
        );

        // Normal
        assert_eq!(
            IntervalDT::try_from_dhms(0, 0, 0, 0, 0)
                .unwrap()
                .sub_time(Time::try_from_hms(1, 2, 3, 4).unwrap())
                .unwrap(),
            -IntervalDT::try_from_dhms(0, 1, 2, 3, 4).unwrap()
        );
    }

    fn test_extract_ym(negate: bool, year: u32, month: u32) {
        let interval = if negate {
            IntervalYM::try_from_ym(year, month).unwrap().negate()
        } else {
            IntervalYM::try_from_ym(year, month).unwrap()
        };

        let modifier = if negate { -1 } else { 1 };

        assert_eq!(year as i32 * modifier, interval.year().unwrap());
        assert_eq!(month as i32 * modifier, interval.month().unwrap());

        assert!(interval.hour().is_none());
        assert!(interval.day().is_none());
        assert!(interval.minute().is_none());
        assert!(interval.second().is_none());
    }

    #[test]
    fn test_interval_ym_extract() {
        test_extract_ym(false, 0, 0);
        test_extract_ym(false, 0, 1);
        test_extract_ym(false, 1, 1);
        test_extract_ym(false, 1234, 11);
        test_extract_ym(false, 178000000, 0);
        test_extract_ym(true, 0, 1);
        test_extract_ym(true, 1, 1);
        test_extract_ym(true, 1234, 11);
        test_extract_ym(true, 178000000, 0);
    }

    #[allow(clippy::float_cmp)]
    fn test_extract_dt(negate: bool, day: u32, hour: u32, min: u32, sec: u32, usec: u32) {
        let interval = if negate {
            IntervalDT::try_from_dhms(day, hour, min, sec, usec)
                .unwrap()
                .negate()
        } else {
            IntervalDT::try_from_dhms(day, hour, min, sec, usec).unwrap()
        };

        let modifier = if negate { -1 } else { 1 };

        assert_eq!(day as i32 * modifier, interval.day().unwrap());
        assert_eq!(hour as i32 * modifier, interval.hour().unwrap());
        assert_eq!(min as i32 * modifier, interval.minute().unwrap());
        assert_eq!(
            modifier as f64 * (sec as f64 + (usec as f64) / 1_000_000f64),
            interval.second().unwrap()
        );
        assert!(interval.year().is_none());
        assert!(interval.month().is_none());
    }

    #[test]
    fn test_interval_dt_extract() {
        test_extract_dt(false, 0, 0, 0, 0, 0);
        test_extract_dt(false, 0, 0, 0, 0, 1);
        test_extract_dt(false, 1, 0, 0, 0, 1);
        test_extract_dt(false, 9999, 23, 59, 59, 999999);
        test_extract_dt(false, 100000000, 0, 0, 0, 0);
        test_extract_dt(true, 0, 0, 0, 0, 1);
        test_extract_dt(true, 1, 0, 0, 0, 1);
        test_extract_dt(true, 9999, 23, 59, 59, 999999);
        test_extract_dt(true, 9999, 23, 59, 59, 375473);
        test_extract_dt(true, 100000000, 0, 0, 0, 0);
    }
}
