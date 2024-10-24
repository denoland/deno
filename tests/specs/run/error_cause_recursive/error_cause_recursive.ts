const x = new Error("foo");
const y = new Error("bar", { cause: x });
x.cause = y;
throw y;
