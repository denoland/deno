// Test file to verify error handling in before/after hooks
const logs: string[] = [];

// Test beforeAll hook throwing error
Deno.test.beforeAll(() => {
  logs.push("beforeAll hook running");
  throw new Error("beforeAll hook failed");
});

// This should not run because beforeAll failed
Deno.test.beforeEach(() => {
  logs.push("beforeEach hook running");
});

// This should not run because beforeAll failed
Deno.test("test should not run", () => {
  logs.push("test running");
  console.log("This test should not execute");
});

// This should not run because beforeAll failed
Deno.test.afterEach(() => {
  logs.push("afterEach hook running");
});

// This should not run because beforeAll failed
Deno.test.afterAll(() => {
  logs.push("afterAll hook running");
});