const foo = new Error("foo");
const bar = new Error("bar", { cause: foo });
foo.cause = bar;

const qux = new Error("qux");
const quux = new Error("quux", { cause: qux });
qux.cause = quux;

throw new AggregateError([bar, quux]);
