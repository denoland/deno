// Test file to verify error handling in afterAll hooks

Deno.test.beforeAll(() => {
  console.log("beforeAll executed");
  
  return () => {
    console.log("beforeAll cleanup executed");
  };
});

Deno.test.beforeEach(() => {
  console.log("beforeEach executed");
});

Deno.test.afterEach(() => {
  console.log("afterEach executed");
});

// First afterAll hook that throws
Deno.test.afterAll(() => {
  console.log("afterAll 1 executed");
  throw new Error("afterAll 1 failed");
});

// Second afterAll hook that should still run
Deno.test.afterAll(() => {
  console.log("afterAll 2 executed");
  console.log("✅ Hook execution continued despite afterAll error");
});

// Third afterAll hook that should still run
Deno.test.afterAll(() => {
  console.log("afterAll 3 executed");
});

Deno.test("first test", () => {
  console.log("test executed");
});

Deno.test("second test", () => {
  console.log("second test executed");
});
