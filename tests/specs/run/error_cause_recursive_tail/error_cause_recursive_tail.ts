const foo = new Error("foo");
const bar = new Error("bar", { cause: foo });
const baz = new Error("baz", { cause: bar });
foo.cause = bar;
throw baz;
