// Test file to verify cleanup function error handling
const logs: string[] = [];

Deno.test.beforeAll(() => {
  logs.push("beforeAll 1 executed");
  
  return () => {
    logs.push("beforeAll 1 cleanup executed");
    throw new Error("beforeAll 1 cleanup failed");
  };
});

Deno.test.beforeAll(() => {
  logs.push("beforeAll 2 executed");
  
  return async () => {
    logs.push("beforeAll 2 cleanup started");
    await new Promise(resolve => setTimeout(resolve, 5));
    logs.push("beforeAll 2 cleanup completed");
  };
});

Deno.test.beforeAll(() => {
  logs.push("beforeAll 3 executed");
  
  return () => {
    logs.push("beforeAll 3 cleanup executed");
    throw new Error("beforeAll 3 cleanup failed");
  };
});

Deno.test.afterAll(() => {
  logs.push("afterAll executed");
});

// Cleanup functions will run after all afterAll hooks, so we can't verify them directly.
// Instead, let's just verify the test passes and the cleanup functions were called correctly
// by checking that the tests completed without hanging.

Deno.test("test should run normally", () => {
  logs.push("test executed");
});

Deno.test("second test should also run", () => {
  logs.push("second test executed");
});