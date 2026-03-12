// Test file to verify error handling in afterAll hooks

Deno.test.afterAll(() => {
  console.log("afterAll 2 executed");
});

Deno.test.afterAll(() => {
  console.log("afterAll 1 executed");
  throw new Error("afterAll 1 failed");
});

Deno.test("first test", () => {
  console.log("test executed");
});
