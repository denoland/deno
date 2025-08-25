// Test file to verify error handling in afterEach hooks
const logs: string[] = [];

Deno.test.beforeAll(() => {
  logs.push("beforeAll executed");
});

let testCount = 0;

Deno.test.beforeEach(() => {
  testCount++;
  logs.push(`beforeEach executed for test ${testCount}`);
});

Deno.test.afterEach(() => {
  logs.push(`afterEach executed for test ${testCount}`);
  
  // Throw error on second test
  if (testCount === 2) {
    throw new Error("afterEach hook failed on second test");
  }
});

Deno.test.afterAll(() => {
  logs.push("afterAll executed");
  console.log("Final logs:", logs);
});

Deno.test("first test", () => {
  logs.push("first test executed");
});

// This test should succeed but afterEach throws
Deno.test("second test", () => {
  logs.push("second test executed");
});

// This test should still run
Deno.test("third test", () => {
  logs.push("third test executed");
});