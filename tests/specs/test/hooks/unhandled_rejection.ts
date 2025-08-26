// Test file to verify unhandled promise rejections in hooks

Deno.test.beforeEach(async () => {
  console.log("beforeEach executed");

  // Create an unhandled promise rejection
  Promise.reject(new Error("Unhandled rejection in beforeEach"));

  // Wait a bit to let the rejection propagate
  await new Promise((resolve) => setTimeout(resolve, 10));
});

Deno.test("test with unhandled rejection in beforeEach", () => {
  console.log("test executed");
});

// Test unhandled rejection in afterAll
Deno.test.afterAll(() => {
  console.log("afterAll with rejection executed");
  Promise.reject(new Error("Unhandled rejection in afterAll"));
});
