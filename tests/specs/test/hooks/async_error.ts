// Test file to verify async error handling in hooks

Deno.test.beforeAll(async () => {
  console.log("beforeAll started");
  await new Promise((resolve) => setTimeout(resolve, 10));
  console.log("beforeAll completed");
});

let testCount = 0;

Deno.test.beforeEach(async () => {
  testCount++;
  console.log(`beforeEach started for test ${testCount}`);

  if (testCount === 2) {
    // Async rejection
    await new Promise((_, reject) => {
      setTimeout(() => reject(new Error("Async error in beforeEach")), 5);
    });
  }

  console.log(`beforeEach completed for test ${testCount}`);
});

Deno.test.afterEach(async () => {
  console.log(`afterEach started for test ${testCount}`);

  if (testCount === 3) {
    // Async rejection in afterEach
    await Promise.reject(new Error("Async error in afterEach"));
  }

  console.log(`afterEach completed for test ${testCount}`);
});

Deno.test.afterAll(async () => {
  console.log("afterAll started");
  await new Promise((resolve) => setTimeout(resolve, 5));
  console.log("afterAll completed");
});

Deno.test("first test", () => {
  console.log("first test executed");
});

Deno.test("second test (beforeEach fails)", () => {
  console.log("second test executed");
});

Deno.test("third test (afterEach fails)", () => {
  console.log("third test executed");
});
