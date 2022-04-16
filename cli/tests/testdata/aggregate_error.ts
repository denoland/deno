const aggregateError = new AggregateError([
  new Error("Error message 1."),
  new Error("Error message 2."),
], "Multiple errors.");
console.log(aggregateError.stack);
console.log();
console.log(aggregateError);
console.log();
throw aggregateError;
