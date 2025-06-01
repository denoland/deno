console.log("Temporal.Now", Temporal.Now.instant());
console.log(
  "Temporal.Instant",
  Temporal.Instant.from("1969-07-20T20:17Z"),
);
console.log(
  "Temporal.ZonedDateTime",
  Temporal.ZonedDateTime.from({
    timeZone: "America/Los_Angeles",
    year: 1995,
    month: 12,
    day: 7,
    hour: 3,
    minute: 24,
    second: 30,
    millisecond: 0,
    microsecond: 3,
    nanosecond: 500,
  }),
);
console.log(
  "Temporal.PlainDate",
  Temporal.PlainDate.from({ year: 2006, month: 8, day: 24 }),
);
console.log(
  "Temporal.PlainTime",
  Temporal.PlainTime.from({
    hour: 19,
    minute: 39,
    second: 9,
    millisecond: 68,
    microsecond: 346,
    nanosecond: 205,
  }),
);
console.log(
  "Temporal.PlainDateTime",
  Temporal.PlainDateTime.from({
    year: 1995,
    month: 12,
    day: 7,
    hour: 15,
  }),
);
console.log(
  "Temporal.PlainYearMonth",
  Temporal.PlainYearMonth.from({ year: 2020, month: 10 }),
);
console.log(
  "Temporal.PlainMonthDay",
  Temporal.PlainMonthDay.from({ month: 7, day: 14 }),
);
console.log(
  "Temporal.Duration",
  Temporal.Duration.from({
    hours: 130,
    minutes: 20,
  }),
);
