Deno.test("aggregate", function () {
  const error1 = new Error("Error 1");
  const error2 = new Error("Error 2");

  throw new AggregateError([error1, error2]);
});
