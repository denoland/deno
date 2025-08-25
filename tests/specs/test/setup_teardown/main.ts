// Test file to verify the setup/teardown functionality with multiple hooks
const logs: string[] = [];

// Multiple beforeAll hooks
Deno.test.beforeAll(() => {
  logs.push("beforeAll 1");
});

Deno.test.beforeAll(() => {
  logs.push("beforeAll 2");
  return () => {
    logs.push("beforeAll 2 cleanup");
  };
});

Deno.test.beforeAll(() => {
  logs.push("beforeAll 3");
  return () => {
    logs.push("beforeAll 3 cleanup");
  };
});

// Multiple beforeEach hooks
Deno.test.beforeEach(() => {
  logs.push("beforeEach 1");
});

Deno.test.beforeEach(() => {
  logs.push("beforeEach 2");
});

// Multiple afterEach hooks
Deno.test.afterEach(() => {
  logs.push("afterEach 1");
});

Deno.test.afterEach(() => {
  logs.push("afterEach 2");
});

// Multiple afterAll hooks
Deno.test.afterAll(() => {
  logs.push("afterAll 1");
});

Deno.test.afterAll(() => {
  logs.push("afterAll 2");
});

// Verification afterAll hook
Deno.test.afterAll(() => {
  console.log("Final logs:", logs);
  
  // Expected order for multiple hooks:
  // beforeAll 1, beforeAll 2, beforeAll 3,
  // beforeEach 1, beforeEach 2, test 1, afterEach 1, afterEach 2,
  // beforeEach 1, beforeEach 2, test 2, afterEach 1, afterEach 2,
  // beforeEach 1, beforeEach 2, test 3, afterEach 1, afterEach 2,
  // afterAll 1, afterAll 2, beforeAll 2 cleanup, beforeAll 3 cleanup
  
  const expectedBeforeAfterAll = [
    "beforeAll 1", "beforeAll 2", "beforeAll 3",
    "beforeEach 1", "beforeEach 2", "test 1", "afterEach 1", "afterEach 2",
    "beforeEach 1", "beforeEach 2", "test 2", "afterEach 1", "afterEach 2", 
    "beforeEach 1", "beforeEach 2", "test 3", "afterEach 1", "afterEach 2"
  ];
  
  // Check that the main sequence is correct (before afterAll hooks run)
  const actualSequence = logs.slice(0, expectedBeforeAfterAll.length);
  console.log("Expected sequence:", expectedBeforeAfterAll);
  console.log("Actual sequence:", actualSequence);
  
  if (JSON.stringify(actualSequence) === JSON.stringify(expectedBeforeAfterAll)) {
    console.log("✅ Hook execution order is correct!");
  } else {
    console.log("❌ Hook execution order is incorrect!");
    throw new Error("Hook execution order mismatch");
  }
});

Deno.test("first test", () => {
  logs.push("test 1");
  console.log("Logs after test 1:", logs);
});

Deno.test("second test", () => {
  logs.push("test 2");
  console.log("Logs after test 2:", logs);
});

Deno.test("third test", () => {
  logs.push("test 3");
});