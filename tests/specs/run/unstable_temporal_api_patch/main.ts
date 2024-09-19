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
