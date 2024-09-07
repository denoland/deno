console.log("Temporal.ZonedDateTime (Unix epoch, UTC)");
const zoned = new Temporal.ZonedDateTime(0n, "UTC");
console.log("era", zoned.era);
console.log("eraYear", zoned.eraYear);
console.log("year", zoned.year);
console.log("month", zoned.month);
console.log("monthCode", zoned.monthCode);
console.log("day", zoned.day);
console.log("hour", zoned.hour);
console.log("minute", zoned.minute);
console.log("second", zoned.second);
console.log("millisecond", zoned.millisecond);
console.log("microsecond", zoned.microsecond);
console.log("nanosecond", zoned.nanosecond);
console.log("timeZoneId", zoned.timeZoneId);
console.log("calendarId", zoned.calendarId);
console.log("dayOfWeek", zoned.dayOfWeek);
console.log("dayOfYear", zoned.dayOfYear);
console.log("weekOfYear", zoned.weekOfYear);
console.log("yearOfWeek", zoned.yearOfWeek);
console.log("hoursInDay", zoned.hoursInDay);
console.log("daysInWeek", zoned.daysInWeek);
console.log("daysInMonth", zoned.daysInMonth);
console.log("daysInYear", zoned.daysInYear);
console.log("monthsInYear", zoned.monthsInYear);
console.log("inLeapYear", zoned.inLeapYear);
console.log("offsetNanoseconds", zoned.offsetNanoseconds);
console.log("offset", zoned.offset);
console.log("epochMilliseconds", zoned.epochMilliseconds);
console.log("epochNanoseconds", zoned.epochNanoseconds);

console.log("Temporal");
console.log(Object.getOwnPropertyNames(Temporal).sort());

console.log("Temporal.Now");
console.log(Object.getOwnPropertyNames(Temporal.Now).sort());

console.log("Temporal.Instant.prototype");
console.log(Object.getOwnPropertyNames(Temporal.Instant.prototype).sort());

console.log("Temporal.ZonedDateTime.prototype");
console.log(
  Object.getOwnPropertyNames(Temporal.ZonedDateTime.prototype).sort(),
);

console.log("Temporal.PlainDate.prototype");
console.log(Object.getOwnPropertyNames(Temporal.PlainDate.prototype).sort());

console.log("Temporal.PlainTime.prototype");
console.log(Object.getOwnPropertyNames(Temporal.PlainTime.prototype).sort());

console.log("Temporal.PlainDateTime.prototype");
console.log(
  Object.getOwnPropertyNames(Temporal.PlainDateTime.prototype).sort(),
);

console.log("Temporal.PlainYearMonth.prototype");
console.log(
  Object.getOwnPropertyNames(Temporal.PlainYearMonth.prototype).sort(),
);

console.log("Temporal.PlainMonthDay.prototype");
console.log(
  Object.getOwnPropertyNames(Temporal.PlainMonthDay.prototype).sort(),
);

console.log("Temporal.Duration.prototype");
console.log(Object.getOwnPropertyNames(Temporal.Duration.prototype).sort());
