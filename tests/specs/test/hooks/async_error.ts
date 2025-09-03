const logs: string[] = [];

Deno.test.beforeAll(async () => {
  logs.push("beforeAll started");
  await new Promise((resolve) => setTimeout(resolve, 10));
  logs.push("beforeAll completed");
});

let testCount = 0;

Deno.test.beforeEach(async () => {
  testCount++;
  logs.push(`beforeEach started for test ${testCount}`);

  if (testCount === 2) {
    // Async rejection
    await new Promise((_, reject) => {
      setTimeout(() => reject(new Error("Async error in beforeEach")), 5);
    });
  }

  logs.push(`beforeEach completed for test ${testCount}`);
});

Deno.test.afterEach(async () => {
  logs.push(`afterEach started for test ${testCount}`);

  if (testCount === 3) {
    // Async rejection in afterEach
    await Promise.reject(new Error("Async error in afterEach"));
  }

  logs.push(`afterEach completed for test ${testCount}`);
});

Deno.test.afterAll(async () => {
  logs.push("afterAll started");
  await new Promise((resolve) => setTimeout(resolve, 5));
  logs.push("afterAll completed");
});

Deno.test("first test", () => {
  logs.push("first test executed");
});

Deno.test("second test (beforeEach fails)", () => {
  logs.push("second test executed");
});

Deno.test("third test (afterEach fails)", () => {
  logs.push("third test executed");
});

globalThis.onunload = () => {
  console.log(logs);
};
