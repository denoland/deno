// Test file to verify cleanup function error handling

Deno.test.beforeAll(() => {
  console.log("beforeAll 1 executed");
  
  return () => {
    console.log("beforeAll 1 cleanup executed");
    throw new Error("beforeAll 1 cleanup failed");
  };
});

Deno.test.beforeAll(() => {
  console.log("beforeAll 2 executed");
  
  return async () => {
    console.log("beforeAll 2 cleanup started");
    await new Promise(resolve => setTimeout(resolve, 5));
    console.log("beforeAll 2 cleanup completed");
  };
});

Deno.test.beforeAll(() => {
  console.log("beforeAll 3 executed");
  
  return () => {
    console.log("beforeAll 3 cleanup executed");
    throw new Error("beforeAll 3 cleanup failed");
  };
});

Deno.test.afterAll(() => {
  console.log("afterAll executed");
});

// Cleanup functions will run after all afterAll hooks, so we can't verify them directly.
// Instead, let's just verify the test passes and the cleanup functions were called correctly
// by checking that the tests completed without hanging.

Deno.test("test should run normally", () => {
  console.log("test executed");
});

Deno.test("second test should also run", () => {
  console.log("second test executed");
});