function foo() {
  return new AggregateError([new Error("foo"), "bar"], "hello");
}

const err = foo();

console.log(Deno.inspect(err, { colors: true }));
