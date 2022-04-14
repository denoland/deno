// deno-lint-ignore-file
const a: string = "string";

const aggregateError = new AggregateError([
  new Error("Error message 1."),
  new Error("Error message 2."),
], "Multiple errors.");

console.error(aggregateError);

throw aggregateError;
