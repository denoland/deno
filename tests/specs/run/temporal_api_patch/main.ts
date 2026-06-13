console.log(Temporal.Now.timeZoneId());
// @ts-expect-error: undefined check
console.log(Temporal.Now.timeZone);

const zoned = new Temporal.ZonedDateTime(0n, "UTC");
console.log(zoned.calendarId);
console.log(zoned.timeZoneId);
// @ts-expect-error: undefined check
console.log(zoned.calendar);
// @ts-expect-error: undefined check
console.log(zoned.timeZone);

const duration = Temporal.Duration.from("P1DT6H30M");
console.log(duration.toLocaleString("en-US"));

const transition = Temporal.ZonedDateTime.from(
  "2020-01-01T00:00:00-05:00[America/New_York]",
).getTimeZoneTransition("next");
console.log(transition);
