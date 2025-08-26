let testCount = 0;

Deno.test.beforeEach(() => {
  testCount++;
  console.log(`beforeEach executed for test ${testCount}`);

  if (testCount === 3) {
    throw new Error("beforeEach failed for test 3");
  }
});

Deno.test.afterEach(() => {
  console.log(`afterEach executed for test ${testCount}`);

  if (testCount === 4) {
    throw new Error("afterEach failed for test 4");
  }
});

Deno.test("test 1 - should pass", () => {
  console.log("test 1 executed");
});

Deno.test("test 2 - should pass", () => {
  console.log("test 2 executed");
});

Deno.test("test 3 - beforeEach fails", () => {
  console.log("test 3 executed");
});

Deno.test("test 4 - afterEach fails", () => {
  console.log("test 4 executed");
});

Deno.test("test 5 - test itself fails", () => {
  console.log("test 5 executed");
  throw new Error("test 5 failed");
});
