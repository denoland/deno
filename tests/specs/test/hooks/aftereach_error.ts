let testCount = 0;

Deno.test.beforeEach(() => {
  testCount++;
  console.log(`beforeEach executed for test ${testCount}`);
});

Deno.test.afterEach(() => {
  console.log(`afterEach executed for test ${testCount}`);

  if (testCount === 1) {
    throw new Error("afterEach hook failed on second test");
  }
});

Deno.test("first test", () => {
  console.log("first test executed");
});

Deno.test("second test", () => {
  console.log("second test executed");
});
