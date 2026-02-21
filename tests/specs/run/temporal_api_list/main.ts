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
