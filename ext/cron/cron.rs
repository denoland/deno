// Copyright 2018-2026 the Deno authors. MIT license.

use std::str::FromStr;

use chrono::DateTime;
use chrono::Datelike;
use chrono::Duration;
use chrono::TimeZone;
use chrono::Timelike;
use chrono::Utc;
use chrono::Weekday;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
  minutes: u64,
  hours: u32,
  days_of_month: DaysOfMonth,
  months: u16,
  days_of_week: DaysOfWeek,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DaysOfMonth {
  Any,
  Days(u32),
  Last { offset: u32, weekday: bool },
  ClosestWeekday(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DaysOfWeek {
  Any,
  Days(u8),
  Last(u32),
  Nth { day: u32, nth: u32 },
}

#[derive(Debug, Clone, Copy)]
enum Field {
  Minute,
  Hour,
  DayOfMonth,
  Month,
  DayOfWeek,
}

impl Field {
  fn min(self) -> u32 {
    match self {
      Self::Minute | Self::Hour => 0,
      Self::DayOfMonth | Self::Month => 1,
      Self::DayOfWeek => 0,
    }
  }

  fn max(self) -> u32 {
    match self {
      Self::Minute => 59,
      Self::Hour => 23,
      Self::DayOfMonth => 31,
      Self::Month => 12,
      Self::DayOfWeek => 6,
    }
  }

  fn max_step(self) -> u32 {
    match self {
      Self::Minute => 59,
      Self::Hour => 23,
      Self::DayOfMonth => 30,
      Self::Month => 11,
      Self::DayOfWeek => 6,
    }
  }

  fn parse_value(self, input: &str) -> Result<u32, ParseError> {
    match self {
      Self::Month => parse_month(input),
      Self::DayOfWeek => parse_day_of_week(input),
      _ => parse_number(input, self.min(), self.max()),
    }
  }
}

impl FromStr for Schedule {
  type Err = ParseError;

  fn from_str(input: &str) -> Result<Self, Self::Err> {
    if input.trim() != input {
      return Err(ParseError);
    }
    if input
      .chars()
      .any(|c| c.is_ascii_whitespace() && c != ' ' && c != '\t')
    {
      return Err(ParseError);
    }
    let fields = input
      .split([' ', '\t'])
      .filter(|field| !field.is_empty())
      .collect::<Vec<_>>();
    if fields.len() != 5 {
      return Err(ParseError);
    }

    Ok(Self {
      minutes: parse_field(fields[0], Field::Minute)?,
      hours: parse_field(fields[1], Field::Hour)? as u32,
      days_of_month: parse_days_of_month(fields[2])?,
      months: parse_field(fields[3], Field::Month)? as u16,
      days_of_week: parse_days_of_week(fields[4])?,
    })
  }
}

impl Schedule {
  pub fn next_after(&self, date: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let mut next = date
      .with_nanosecond(0)?
      .with_second(0)?
      .checked_add_signed(Duration::minutes(1))?;
    let end = next.checked_add_signed(Duration::days(366 * 5))?;

    while next <= end {
      if self.contains(next) {
        return Some(next);
      }

      if (self.months & (1u16 << next.month())) == 0 {
        next = self.next_matching_month_start(next)?;
      } else if !self.matches_day(next) {
        next = next
          .checked_add_signed(Duration::days(1))?
          .with_hour(0)?
          .with_minute(0)?;
      } else if (self.hours & (1u32 << next.hour())) == 0 {
        next = self.next_matching_hour_start(next)?;
      } else {
        next = self.next_matching_minute_start(next)?;
      }
    }

    None
  }

  fn contains(&self, date: DateTime<Utc>) -> bool {
    (self.minutes & (1u64 << date.minute())) != 0
      && (self.hours & (1u32 << date.hour())) != 0
      && (self.months & (1u16 << date.month())) != 0
      && self.matches_day(date)
  }

  fn matches_day(&self, date: DateTime<Utc>) -> bool {
    match (self.days_of_month.is_any(), self.days_of_week.is_any()) {
      (true, true) => true,
      (true, false) => self.days_of_week.contains(date),
      (false, true) => self.days_of_month.contains(date),
      (false, false) => {
        self.days_of_month.contains(date) || self.days_of_week.contains(date)
      }
    }
  }

  fn next_matching_month_start(
    &self,
    date: DateTime<Utc>,
  ) -> Option<DateTime<Utc>> {
    let mut year = date.year();
    let mut month = date.month().checked_add(1)?;

    loop {
      while month <= 12 {
        if (self.months & (1u16 << month)) != 0 {
          return ymd_hms(year, month, 1, 0, 0, 0);
        }
        month += 1;
      }

      year = year.checked_add(1)?;
      month = 1;
    }
  }

  fn next_matching_hour_start(
    &self,
    date: DateTime<Utc>,
  ) -> Option<DateTime<Utc>> {
    for hour in date.hour()..=23 {
      if (self.hours & (1u32 << hour)) != 0 {
        return date.with_hour(hour)?.with_minute(0);
      }
    }

    date
      .checked_add_signed(Duration::days(1))?
      .with_hour(0)?
      .with_minute(0)
  }

  fn next_matching_minute_start(
    &self,
    date: DateTime<Utc>,
  ) -> Option<DateTime<Utc>> {
    for minute in date.minute()..=59 {
      if (self.minutes & (1u64 << minute)) != 0 {
        return date.with_minute(minute);
      }
    }

    date.checked_add_signed(Duration::hours(1))?.with_minute(0)
  }
}

impl DaysOfMonth {
  fn is_any(&self) -> bool {
    matches!(self, Self::Any)
  }

  fn contains(&self, date: DateTime<Utc>) -> bool {
    let day = date.day();
    let days_in_month = days_in_month(date.year(), date.month());
    match *self {
      Self::Any => true,
      Self::Days(days) => (days & (1u32 << day)) != 0,
      Self::Last { offset, weekday } => {
        let Some(target) = days_in_month.checked_sub(offset) else {
          return false;
        };
        if target == 0 {
          return false;
        }
        if weekday {
          Some(day) == closest_weekday(date.year(), date.month(), target)
        } else {
          day == target
        }
      }
      Self::ClosestWeekday(target) => {
        Some(day) == closest_weekday(date.year(), date.month(), target)
      }
    }
  }
}

impl DaysOfWeek {
  fn is_any(&self) -> bool {
    matches!(self, Self::Any)
  }

  fn contains(&self, date: DateTime<Utc>) -> bool {
    let weekday = date.weekday().num_days_from_sunday();
    match *self {
      Self::Any => true,
      Self::Days(days) => (days & (1u8 << weekday)) != 0,
      Self::Last(day) => {
        weekday == day
          && date.day() + 7 > days_in_month(date.year(), date.month())
      }
      Self::Nth { day, nth } => {
        weekday == day && ((date.day() - 1) / 7) + 1 == nth
      }
    }
  }
}

fn parse_field(input: &str, field: Field) -> Result<u64, ParseError> {
  if input == "*" {
    return Ok(bit_range(field.min(), field.max()));
  }

  let mut bits = 0;
  for part in input.split(',') {
    if part.is_empty() {
      return Err(ParseError);
    }
    bits |= parse_part(part, field)?;
  }
  if bits == 0 {
    return Err(ParseError);
  }
  Ok(bits)
}

fn parse_part(input: &str, field: Field) -> Result<u64, ParseError> {
  let (base, step) = match input.split_once('/') {
    Some((base, step)) => (base, Some(parse_step(step, field)?)),
    None => (input, None),
  };

  let (start, end) = if base == "*" {
    (field.min(), field.max())
  } else if let Some((start, end)) = base.split_once('-') {
    (field.parse_value(start)?, field.parse_value(end)?)
  } else {
    let start = field.parse_value(base)?;
    (start, step.map(|_| field.max()).unwrap_or(start))
  };

  Ok(match step {
    Some(step) => stepped_bits(start, end, step, field),
    None => range_bits(start, end, field),
  })
}

fn parse_days_of_month(input: &str) -> Result<DaysOfMonth, ParseError> {
  if input == "*" {
    return Ok(DaysOfMonth::Any);
  }

  if input == "L" {
    return Ok(DaysOfMonth::Last {
      offset: 0,
      weekday: false,
    });
  }

  if input == "LW" {
    return Ok(DaysOfMonth::Last {
      offset: 0,
      weekday: true,
    });
  }

  if let Some(offset) = input.strip_prefix("L-") {
    let (offset, weekday) = match offset.strip_suffix('W') {
      Some(offset) => (offset, true),
      None => (offset, false),
    };
    return Ok(DaysOfMonth::Last {
      offset: parse_number(offset, 1, 30)?,
      weekday,
    });
  }

  if let Some(day) = input.strip_suffix('W') {
    return Ok(DaysOfMonth::ClosestWeekday(parse_number(day, 1, 31)?));
  }

  Ok(DaysOfMonth::Days(
    parse_field(input, Field::DayOfMonth)? as u32
  ))
}

fn parse_days_of_week(input: &str) -> Result<DaysOfWeek, ParseError> {
  if input == "*" {
    return Ok(DaysOfWeek::Any);
  }

  if input == "L" {
    return Ok(DaysOfWeek::Days(1u8 << 6));
  }

  if let Some(day) = input.strip_suffix('L') {
    return Ok(DaysOfWeek::Last(Field::DayOfWeek.parse_value(day)?));
  }

  if let Some((day, nth)) = input.split_once('#') {
    return Ok(DaysOfWeek::Nth {
      day: Field::DayOfWeek.parse_value(day)?,
      nth: parse_number(nth, 1, 5)?,
    });
  }

  Ok(DaysOfWeek::Days(parse_field(input, Field::DayOfWeek)? as u8))
}

fn parse_step(input: &str, field: Field) -> Result<u32, ParseError> {
  parse_number(input, 1, field.max_step())
}

fn parse_number(input: &str, min: u32, max: u32) -> Result<u32, ParseError> {
  if input.is_empty()
    || !input.bytes().all(|b| b.is_ascii_digit())
    || input.len() > 2
  {
    return Err(ParseError);
  }
  let value = input.parse::<u32>().map_err(|_| ParseError)?;
  if value < min || value > max {
    return Err(ParseError);
  }
  Ok(value)
}

fn parse_month(input: &str) -> Result<u32, ParseError> {
  match input.to_ascii_uppercase().as_str() {
    "JAN" => Ok(1),
    "FEB" => Ok(2),
    "MAR" => Ok(3),
    "APR" => Ok(4),
    "MAY" => Ok(5),
    "JUN" => Ok(6),
    "JUL" => Ok(7),
    "AUG" => Ok(8),
    "SEP" => Ok(9),
    "OCT" => Ok(10),
    "NOV" => Ok(11),
    "DEC" => Ok(12),
    _ => parse_number(input, 1, 12),
  }
}

fn parse_day_of_week(input: &str) -> Result<u32, ParseError> {
  match input.to_ascii_uppercase().as_str() {
    "SUN" => Ok(0),
    "MON" => Ok(1),
    "TUE" => Ok(2),
    "WED" => Ok(3),
    "THU" => Ok(4),
    "FRI" => Ok(5),
    "SAT" => Ok(6),
    _ => match parse_number(input, 1, 7)? {
      7 => Ok(6),
      day => Ok(day - 1),
    },
  }
}

fn range_bits(start: u32, end: u32, field: Field) -> u64 {
  if start <= end {
    bit_range(start, end)
  } else {
    bit_range(start - 1, field.max()) | bit_range(field.min(), end)
  }
}

fn stepped_bits(start: u32, end: u32, step: u32, field: Field) -> u64 {
  let values = if start <= end {
    (start..=end).collect::<Vec<_>>()
  } else {
    (start..=field.max())
      .chain(field.min()..=end)
      .collect::<Vec<_>>()
  };
  values
    .into_iter()
    .step_by(step as usize)
    .fold(0, |bits, value| bits | (1u64 << value))
}

fn bit_range(start: u32, end: u32) -> u64 {
  (start..=end).fold(0, |bits, value| bits | (1u64 << value))
}

fn days_in_month(year: i32, month: u32) -> u32 {
  match month {
    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
    4 | 6 | 9 | 11 => 30,
    2 if is_leap_year(year) => 29,
    2 => 28,
    _ => unreachable!(),
  }
}

fn ymd_hms(
  year: i32,
  month: u32,
  day: u32,
  hour: u32,
  minute: u32,
  second: u32,
) -> Option<DateTime<Utc>> {
  Utc
    .with_ymd_and_hms(year, month, day, hour, minute, second)
    .single()
}

fn is_leap_year(year: i32) -> bool {
  year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn closest_weekday(year: i32, month: u32, day: u32) -> Option<u32> {
  if day == 0 || day > days_in_month(year, month) {
    return None;
  }

  Some(match ymd_hms(year, month, day, 0, 0, 0)?.weekday() {
    Weekday::Sat if day == 1 => day + 2,
    Weekday::Sat => day - 1,
    Weekday::Sun if day == days_in_month(year, month) => day - 2,
    Weekday::Sun => day + 1,
    _ => day,
  })
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone;

  use super::*;

  fn dt(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
  ) -> DateTime<Utc> {
    Utc
      .with_ymd_and_hms(year, month, day, hour, minute, 0)
      .unwrap()
  }

  fn next(schedule: &str, date: DateTime<Utc>) -> DateTime<Utc> {
    schedule
      .parse::<Schedule>()
      .unwrap()
      .next_after(date)
      .unwrap()
  }

  #[test]
  fn parses_saffron_compatible_valid_fields() {
    for schedule in [
      "* * * * *",
      "0-30/5 1,2,3 1/3 jan-mar mon-fri",
      "5/10 8-18 * * *",
      "59-0 23-0 31-1 12-1 *",
      "0 0 L * *",
      "0 0 LW * *",
      "0 0 L-30W * *",
      "0 0 1W * *",
      "0 0 * * 7L",
      "0 0 * * MON#2",
    ] {
      assert!(
        schedule.parse::<Schedule>().is_ok(),
        "{schedule} should parse"
      );
    }
  }

  #[test]
  fn rejects_malformed_input() {
    for schedule in [
      "",
      " * * * * *",
      "* * * * * ",
      "*\n* * * *",
      "* * * *",
      "* * * * * *",
      "m * * * *",
      "*/0 * * * *",
      "*/60 * * * *",
      "* 24 * * *",
      "* * 0 * *",
      "* * 32 * *",
      "* * ? * *",
      "* * * 0 *",
      "* * * 13 *",
      "* * * * 0",
      "* * * * 8",
      "0 0 L-0 * *",
      "0 0 L-31W * *",
      "0 0 * * MON#0",
      "0 0 * * MON#6",
      "0 0 * * MON,TUE#2",
    ] {
      assert!(
        schedule.parse::<Schedule>().is_err(),
        "{schedule} should be rejected"
      );
    }
  }

  #[test]
  fn computes_next_matching_times() {
    for (schedule, start, expected) in [
      ("* * * * *", dt(2024, 1, 1, 0, 0), dt(2024, 1, 1, 0, 1)),
      ("*/15 * * * *", dt(2024, 1, 1, 0, 0), dt(2024, 1, 1, 0, 15)),
      ("0 0 29 2 *", dt(2023, 3, 1, 0, 0), dt(2024, 2, 29, 0, 0)),
      ("0 0 29 2 *", dt(2025, 3, 1, 0, 0), dt(2028, 2, 29, 0, 0)),
      (
        "0 0 * JAN MON",
        dt(2023, 12, 31, 0, 0),
        dt(2024, 1, 1, 0, 0),
      ),
      ("0 0 * * 1", dt(2024, 1, 6, 0, 0), dt(2024, 1, 7, 0, 0)),
      ("0 0 * * 7", dt(2024, 1, 5, 0, 0), dt(2024, 1, 6, 0, 0)),
      ("0 0 13 * TUE", dt(2024, 5, 13, 0, 0), dt(2024, 5, 14, 0, 0)),
      (
        "59-0 23-0 31-1 12-1 *",
        dt(2020, 12, 30, 23, 58),
        dt(2020, 12, 30, 23, 59),
      ),
    ] {
      assert_eq!(next(schedule, start), expected, "{schedule}");
    }
  }

  #[test]
  fn matches_saffron_plain_wraparound_ranges() {
    for (schedule, date, expected) in [
      ("0 0 1 12-1 *", dt(2020, 11, 1, 0, 0), true),
      ("0 0 1 12-1 *", dt(2020, 12, 1, 0, 0), true),
      ("0 0 1 12-1 *", dt(2020, 1, 1, 0, 0), true),
      ("0 0 1 12-1 *", dt(2020, 10, 1, 0, 0), false),
      ("0 0 * * SAT-SUN", dt(2020, 1, 3, 0, 0), true),
      ("0 0 * * SAT-SUN", dt(2020, 1, 4, 0, 0), true),
      ("0 0 * * SAT-SUN", dt(2020, 1, 5, 0, 0), true),
      ("0 0 * * SAT-SUN", dt(2020, 1, 6, 0, 0), false),
      ("0 20-4 * * *", dt(2020, 1, 1, 19, 0), true),
      ("0 20-4 * * *", dt(2020, 1, 1, 20, 0), true),
      ("0 20-4 * * *", dt(2020, 1, 1, 4, 0), true),
      ("0 20-4 * * *", dt(2020, 1, 1, 5, 0), false),
      ("59-0 * * * *", dt(2020, 1, 1, 0, 58), true),
      ("59-0 * * * *", dt(2020, 1, 1, 0, 59), true),
      ("59-0 * * * *", dt(2020, 1, 1, 0, 0), true),
      ("59-0 * * * *", dt(2020, 1, 1, 0, 1), false),
      ("0 0 31-1 * *", dt(2020, 1, 30, 0, 0), true),
      ("0 0 31-1 * *", dt(2020, 1, 31, 0, 0), true),
      ("0 0 31-1 * *", dt(2020, 1, 1, 0, 0), true),
      ("0 0 31-1 * *", dt(2020, 1, 2, 0, 0), false),
    ] {
      let schedule = schedule.parse::<Schedule>().unwrap();
      assert_eq!(schedule.contains(date), expected, "{schedule:?} {date}");
    }
  }

  #[test]
  fn matches_saffron_frozen_cases() {
    for (schedule, date, expected) in [
      ("0 0 * * 1", dt(2024, 1, 7, 0, 0), true),
      ("0 0 * * 7", dt(2024, 1, 6, 0, 0), true),
      ("0 0 * * SUN", dt(2024, 1, 7, 0, 0), true),
      ("0 0 * * SAT", dt(2024, 1, 6, 0, 0), true),
      ("0 0 * JAN MON", dt(2024, 1, 1, 0, 0), true),
      ("0 0 * JAN MON", dt(2024, 2, 5, 0, 0), false),
      ("*/15,30-59/10 0 * * *", dt(2020, 1, 1, 0, 40), true),
      ("*/15,30-59/10 0 * * *", dt(2020, 1, 1, 0, 35), false),
      ("0 20-4/2 * * *", dt(2020, 1, 1, 22, 0), true),
      ("0 20-4/2 * * *", dt(2020, 1, 1, 3, 0), false),
      ("59-0 23-0 31-1 12-1 *", dt(2020, 12, 31, 23, 59), true),
      ("59-0 23-0 31-1 12-1 *", dt(2020, 11, 1, 0, 0), true),
      ("0 0 * JAN SAT-SUN", dt(2020, 1, 4, 0, 0), true),
      ("0 0 * JAN SAT-SUN", dt(2020, 1, 6, 0, 0), false),
      ("0 0 L FEB *", dt(2024, 2, 29, 0, 0), true),
      ("0 0 L-1 FEB *", dt(2024, 2, 28, 0, 0), true),
      ("0 0 L-1 FEB *", dt(2024, 2, 29, 0, 0), false),
      ("0 0 LW MAY *", dt(2025, 5, 30, 0, 0), true),
      ("0 0 LW MAY *", dt(2025, 5, 31, 0, 0), false),
      ("0 0 L-1W MAY *", dt(2021, 5, 31, 0, 0), true),
      ("0 0 L-30W * *", dt(2021, 5, 3, 0, 0), true),
      ("0 0 L-30W * *", dt(2021, 4, 1, 0, 0), false),
      ("0 0 1W MAY *", dt(2021, 5, 3, 0, 0), true),
      ("0 0 1W MAY *", dt(2022, 5, 2, 0, 0), true),
      ("0 0 * * 7L", dt(2020, 2, 29, 0, 0), true),
      ("0 0 * * 7L", dt(2020, 2, 22, 0, 0), false),
      ("0 0 * * SAT#5", dt(2020, 2, 29, 0, 0), true),
      ("0 0 * * SAT#5", dt(2020, 3, 28, 0, 0), false),
      ("0 0 13 * TUE", dt(2024, 5, 13, 0, 0), true),
      ("0 0 13 * TUE", dt(2024, 5, 14, 0, 0), true),
      ("0 0 13 * TUE", dt(2024, 5, 15, 0, 0), false),
    ] {
      let schedule = schedule.parse::<Schedule>().unwrap();
      assert_eq!(schedule.contains(date), expected, "{schedule:?} {date}");
    }
  }

  #[test]
  fn invalid_derived_dates_do_not_panic_or_match() {
    let schedule = "0 0 L-30W APR *".parse::<Schedule>().unwrap();
    for day in 1..=30 {
      assert!(!schedule.contains(dt(2021, 4, day, 0, 0)));
    }
    assert_eq!(
      schedule.next_after(dt(2021, 4, 1, 0, 0)),
      None,
      "L-30W in a 30-day-only month should never match"
    );

    let schedule = "0 0 31 FEB *".parse::<Schedule>().unwrap();
    assert_eq!(schedule.next_after(dt(2024, 1, 1, 0, 0)), None);
  }
}
