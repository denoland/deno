const error = new AggregateError(
  [
    new AggregateError([new Error("qux1"), new Error("quux1")]),
    new Error("bar1", { cause: new Error("baz1") }),
  ],
  "foo1",
  {
    cause: new AggregateError([
      new AggregateError([new Error("qux2"), new Error("quux2")]),
      new Error("bar2", { cause: new Error("baz2") }),
    ], "foo2"),
  },
);
console.log(error.stack);
console.log();
console.log(error);
console.log();
throw error;
