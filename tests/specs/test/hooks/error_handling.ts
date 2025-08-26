Deno.test.beforeAll(() => {
  console.log("beforeAll hook running");
  throw new Error("beforeAll hook failed");
});

Deno.test.beforeEach(() => {
  console.log("beforeEach should not execute");
});

Deno.test("test should not run", () => {
  console.log("test should not execute");
});

Deno.test.afterEach(() => {
  console.log("afterEach should not execute");
});

Deno.test.afterAll(() => {
  console.log("afterAll hook running");
});
