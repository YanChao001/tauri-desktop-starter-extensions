//! 公历日期计算；年月采用月末截断，减法按日、月、年的顺序执行。

use serde::{Deserialize, Serialize};

const MIN_YEAR: i32 = 1601;
const MAX_YEAR: i32 = 9999;
const INVALID_DATE: &str = "Invalid date";
const OUT_OF_RANGE: &str = "Date out of range";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DateOutput {
    pub total_days: u32,
    pub years: u32,
    pub months: u32,
    pub weeks: u32,
    pub days: u32,
    pub date: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Date {
    year: i32,
    month: u32,
    day: u32,
}

impl Date {
    fn parse(value: &str) -> Result<Self, String> {
        let bytes = value.as_bytes();
        if bytes.len() != 10
            || bytes[4] != b'-'
            || bytes[7] != b'-'
            || !bytes[..4].iter().all(u8::is_ascii_digit)
            || !bytes[5..7].iter().all(u8::is_ascii_digit)
            || !bytes[8..].iter().all(u8::is_ascii_digit)
        {
            return Err(INVALID_DATE.into());
        }
        let number = |digits: &[u8]| {
            digits
                .iter()
                .fold(0, |value, digit| value * 10 + u32::from(digit - b'0'))
        };
        let year = number(&bytes[..4]) as i32;
        let month = number(&bytes[5..7]);
        let day = number(&bytes[8..]);
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return Err(OUT_OF_RANGE.into());
        }
        if !(1..=12).contains(&month) || day == 0 || day > month_days(year, month) {
            return Err(INVALID_DATE.into());
        }
        Ok(Self { year, month, day })
    }

    fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    fn ordinal(self) -> i32 {
        let mut result = days_before_year(self.year) + self.day as i32 - 1;
        for month in 1..self.month {
            result += month_days(self.year, month) as i32;
        }
        result
    }

    fn from_ordinal(ordinal: i32) -> Result<Self, String> {
        if ordinal < days_before_year(MIN_YEAR) || ordinal >= days_before_year(MAX_YEAR + 1) {
            return Err(OUT_OF_RANGE.into());
        }
        let (mut low, mut high) = (MIN_YEAR, MAX_YEAR + 1);
        while low + 1 < high {
            let middle = low + (high - low) / 2;
            if days_before_year(middle) <= ordinal {
                low = middle;
            } else {
                high = middle;
            }
        }
        let mut remaining = ordinal - days_before_year(low);
        let mut month = 1;
        while remaining >= month_days(low, month) as i32 {
            remaining -= month_days(low, month) as i32;
            month += 1;
        }
        Ok(Self {
            year: low,
            month,
            day: remaining as u32 + 1,
        })
    }

    fn add_years(self, years: i32) -> Result<Self, String> {
        let year = self.year + years;
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return Err(OUT_OF_RANGE.into());
        }
        Ok(Self {
            year,
            day: self.day.min(month_days(year, self.month)),
            ..self
        })
    }

    fn add_months(self, months: i32) -> Result<Self, String> {
        let index = self.year * 12 + self.month as i32 - 1 + months;
        let year = index.div_euclid(12);
        let month = index.rem_euclid(12) as u32 + 1;
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return Err(OUT_OF_RANGE.into());
        }
        Ok(Self {
            year,
            month,
            day: self.day.min(month_days(year, month)),
        })
    }

    fn add_days(self, days: i32) -> Result<Self, String> {
        Self::from_ordinal(self.ordinal() + days)
    }
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn month_days(year: i32, month: u32) -> u32 {
    match month {
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn days_before_year(year: i32) -> i32 {
    let previous = year - 1;
    previous * 365 + previous / 4 - previous / 100 + previous / 400
}

fn between(from: Date, to: Date) -> DateOutput {
    let (start, end) = if from <= to { (from, to) } else { (to, from) };
    let total_days = (end.ordinal() - start.ordinal()) as u32;

    // 每种单位从当前锚点一次性增加，避免逐月截断累积改变起始日。
    let mut years = (end.year - start.year) as u32;
    let mut pivot = start.add_years(years as i32).expect("年份来自有效结束日期");
    if pivot > end {
        years -= 1;
        pivot = start.add_years(years as i32).expect("已减少有效年份");
    }
    let mut months = ((end.year - pivot.year) * 12 + end.month as i32 - pivot.month as i32) as u32;
    let mut month_pivot = pivot
        .add_months(months as i32)
        .expect("月份来自有效结束日期");
    if month_pivot > end {
        months -= 1;
        month_pivot = pivot.add_months(months as i32).expect("已减少有效月份");
    }
    let remaining = (end.ordinal() - month_pivot.ordinal()) as u32;
    DateOutput {
        total_days,
        years,
        months,
        weeks: remaining / 7,
        days: remaining % 7,
        date: end.iso(),
    }
}

pub fn difference(from: &str, to: &str) -> Result<DateOutput, String> {
    Ok(between(Date::parse(from)?, Date::parse(to)?))
}

pub fn offset(
    from: &str,
    years: u32,
    months: u32,
    days: u32,
    subtract: bool,
) -> Result<DateOutput, String> {
    if years > 999 || months > 999 || days > 999 {
        return Err("Date offset must be between 0 and 999".into());
    }
    let start = Date::parse(from)?;
    let end = if subtract {
        start
            .add_days(-(days as i32))?
            .add_months(-(months as i32))?
            .add_years(-(years as i32))?
    } else {
        start
            .add_years(years as i32)?
            .add_months(months as i32)?
            .add_days(days as i32)?
    };
    let mut result = between(start, end);
    result.date = end.iso();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(from: &str, to: &str) -> (u32, u32, u32, u32, u32) {
        let result = difference(from, to).unwrap();
        (
            result.years,
            result.months,
            result.weeks,
            result.days,
            result.total_days,
        )
    }

    #[test]
    fn difference_is_unsigned_and_uses_full_calendar_units() {
        assert_eq!(parts("2007-02-28", "2008-01-29"), (0, 11, 0, 1, 335));
        assert_eq!(
            difference("2007-02-28", "2008-01-29"),
            difference("2008-01-29", "2007-02-28")
        );
        assert_eq!(parts("2008-02-29", "2008-03-31"), (0, 1, 0, 2, 31));
    }

    #[test]
    fn difference_clamps_month_ends_without_accumulating_clamps() {
        assert_eq!(parts("2023-01-31", "2023-02-28"), (0, 1, 0, 0, 28));
        assert_eq!(parts("2023-01-31", "2023-03-30"), (0, 1, 4, 2, 58));
        assert_eq!(parts("2023-01-31", "2023-03-31"), (0, 2, 0, 0, 59));
        assert_eq!(parts("2020-02-29", "2021-02-28"), (1, 0, 0, 0, 365));
    }

    #[test]
    fn same_date_and_partial_weeks() {
        assert_eq!(parts("2024-02-29", "2024-02-29"), (0, 0, 0, 0, 0));
        assert_eq!(parts("2024-02-29", "2024-03-14"), (0, 0, 2, 0, 14));
        assert_eq!(parts("2024-02-29", "2024-03-17"), (0, 0, 2, 3, 17));
    }

    #[test]
    fn supported_date_boundaries_and_maximum_difference() {
        assert_eq!(
            parts("1601-01-01", "9999-12-31"),
            (8398, 11, 4, 2, 3_067_670)
        );
        assert_eq!(parts("9998-12-31", "9999-12-31"), (1, 0, 0, 0, 365));
        assert!(offset("9999-12-31", 0, 0, 1, false).is_err());
        assert!(offset("1601-01-01", 0, 0, 1, true).is_err());
        assert!(offset("9999-12-31", 1, 0, 0, false).is_err());
        assert!(offset("1601-01-01", 0, 1, 0, true).is_err());
    }

    #[test]
    fn adding_uses_years_then_months_then_days() {
        assert_eq!(
            offset("2008-01-31", 0, 1, 0, false).unwrap().date,
            "2008-02-29"
        );
        assert_eq!(
            offset("2008-01-31", 0, 1, 10, false).unwrap().date,
            "2008-03-10"
        );
        assert_eq!(
            offset("2020-02-29", 1, 1, 0, false).unwrap().date,
            "2021-03-28"
        );
        assert_eq!(
            offset("2008-03-31", 0, 1, 10, false).unwrap().date,
            "2008-05-10"
        );
    }

    #[test]
    fn subtracting_uses_days_then_months_then_years() {
        assert_eq!(
            offset("2008-03-31", 0, 1, 0, true).unwrap().date,
            "2008-02-29"
        );
        assert_eq!(
            offset("2008-03-10", 0, 1, 10, true).unwrap().date,
            "2008-01-29"
        );
        assert_eq!(
            offset("2007-03-10", 0, 1, 10, true).unwrap().date,
            "2007-01-28"
        );
        assert_eq!(
            offset("2021-03-31", 1, 1, 0, true).unwrap().date,
            "2020-02-28"
        );
    }

    #[test]
    fn dates_are_strict_and_gregorian_century_rules_apply() {
        for invalid in [
            "2024-2-29",
            "2024-02-30",
            "1900-02-29",
            "2100-02-29",
            "2024-00-01",
            "2024-01-00",
            "2024-13-01",
            "2024-04-31",
            "2024-02-29T00:00:00Z",
            "２０２４-02-29",
        ] {
            assert_eq!(difference(invalid, "2024-02-29").unwrap_err(), INVALID_DATE);
        }
        assert!(difference("2000-02-29", "2000-03-01").is_ok());
        assert_eq!(
            difference("1600-12-31", "2024-02-29").unwrap_err(),
            OUT_OF_RANGE
        );
        assert_eq!(
            difference("0001-01-01", "2024-02-29").unwrap_err(),
            OUT_OF_RANGE
        );
    }

    #[test]
    fn offset_limits_and_zero_preserve_the_date() {
        assert_eq!(
            offset("2024-02-29", 0, 0, 0, false).unwrap().date,
            "2024-02-29"
        );
        assert_eq!(
            offset("2024-02-29", 0, 0, 0, true).unwrap().date,
            "2024-02-29"
        );
        assert!(offset("2024-01-01", 1000, 0, 0, false).is_err());
        assert!(offset("2024-01-01", 0, 1000, 0, false).is_err());
        assert!(offset("2024-01-01", 0, 0, u32::MAX, false).is_err());
        assert_eq!(
            offset("2024-01-01", 0, 0, 999, false).unwrap().date,
            "2026-09-26"
        );
    }

    #[test]
    fn ordinal_round_trip_covers_leap_centuries_and_both_boundaries() {
        for year in [1601, 1699, 1700, 1899, 1900, 1999, 2000, 2024, 9999] {
            for month in 1..=12 {
                for day in [1, month_days(year, month)] {
                    let date = Date { year, month, day };
                    assert_eq!(Date::from_ordinal(date.ordinal()).unwrap(), date);
                }
            }
        }
    }
}
