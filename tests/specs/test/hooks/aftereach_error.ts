// Test file to verify error handling in afterEach hooks

Deno.test.beforeAll(() => {
  console.log("beforeAll executed");
});

let testCount = 0;

Deno.test.beforeEach(() => {
  testCount++;
  console.log(`beforeEach executed for test ${testCount}`);
});

Deno.test.afterEach(() => {
  console.log(`afterEach executed for test ${testCount}`);
  
  // Throw error on second test
  if (testCount === 2) {
    throw new Error("afterEach hook failed on second test");
  }
});

Deno.test.afterAll(() => {
  console.log("afterAll executed");
});

Deno.test("first test", () => {
  console.log("first test executed");
});

// This test should succeed but afterEach throws
Deno.test("second test", () => {
  console.log("second test executed");
});

// This test should still run
Deno.test("third test", () => {
  console.log("third test executed");
});