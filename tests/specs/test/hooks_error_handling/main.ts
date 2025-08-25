// Test file to verify error handling in before/after hooks

// Test beforeAll hook throwing error
Deno.test.beforeAll(() => {
  console.log("beforeAll hook running");
  throw new Error("beforeAll hook failed");
});

// This should not run because beforeAll failed
Deno.test.beforeEach(() => {
  console.log("beforeEach hook running");
});

// This should not run because beforeAll failed
Deno.test("test should not run", () => {
  console.log("test running");
  console.log("This test should not execute");
});

// This should not run because beforeAll failed
Deno.test.afterEach(() => {
  console.log("afterEach hook running");
});

// This should not run because beforeAll failed
Deno.test.afterAll(() => {
  console.log("afterAll hook running");
});