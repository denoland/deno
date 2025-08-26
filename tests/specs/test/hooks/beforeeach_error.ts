let testCount = 0;

Deno.test.beforeEach(() => {
  testCount++;
  console.log(`beforeEach executed for test ${testCount}`);

  // Throw error on second test
  if (testCount === 2) {
    throw new Error("beforeEach hook failed on second test");
  }
});

Deno.test.afterEach(() => {
  console.log(`afterEach executed for test ${testCount}`);
});

Deno.test("first test", () => {
  console.log("first test executed");
});

// This test should fail because beforeEach throws
Deno.test("second test", () => {
  console.log("second test executed");
});

// This test should still run
Deno.test("third test", () => {
  console.log("third test executed");
});
