// Test file to verify error handling in afterAll hooks
const logs: string[] = [];

Deno.test.beforeAll(() => {
  logs.push("beforeAll executed");
  
  return () => {
    logs.push("beforeAll cleanup executed");
  };
});

Deno.test.beforeEach(() => {
  logs.push("beforeEach executed");
});

Deno.test.afterEach(() => {
  logs.push("afterEach executed");
});

// First afterAll hook that throws
Deno.test.afterAll(() => {
  logs.push("afterAll 1 executed");
  throw new Error("afterAll 1 failed");
});

// Second afterAll hook that should still run
Deno.test.afterAll(() => {
  logs.push("afterAll 2 executed");
  console.log("Final logs:", logs);
  
  // Verify that despite the first afterAll failing, everything else ran
  const expectedLogs = [
    "beforeAll executed",
    "beforeEach executed",
    "test executed",
    "afterEach executed",
    "beforeEach executed", 
    "second test executed",
    "afterEach executed",
    "afterAll 1 executed",
    "afterAll 2 executed"
  ];
  
  const actualSequence = logs.slice(0, expectedLogs.length);
  
  if (JSON.stringify(actualSequence) === JSON.stringify(expectedLogs)) {
    console.log("✅ Hook execution continued despite afterAll error");
  } else {
    console.log("❌ Hook execution was interrupted by afterAll error");
    console.log("Expected:", expectedLogs);
    console.log("Actual:", actualSequence);
  }
});

// Third afterAll hook that should still run
Deno.test.afterAll(() => {
  logs.push("afterAll 3 executed");
});

Deno.test("first test", () => {
  logs.push("test executed");
});

Deno.test("second test", () => {
  logs.push("second test executed");
});