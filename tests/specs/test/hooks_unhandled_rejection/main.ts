// Test file to verify unhandled promise rejections in hooks
const logs: string[] = [];

Deno.test.beforeAll(() => {
  logs.push("beforeAll executed");
});

Deno.test.beforeEach(async () => {
  logs.push("beforeEach executed");
  
  // Create an unhandled promise rejection
  Promise.reject(new Error("Unhandled rejection in beforeEach"));
  
  // Wait a bit to let the rejection propagate
  await new Promise(resolve => setTimeout(resolve, 10));
});

Deno.test.afterEach(() => {
  logs.push("afterEach executed");
});

Deno.test.afterAll(() => {
  logs.push("afterAll executed");
  console.log("Final logs:", logs);
});

Deno.test("test with unhandled rejection in beforeEach", () => {
  logs.push("test executed");
});

// Test unhandled rejection in afterAll
Deno.test.afterAll(() => {
  logs.push("afterAll with rejection executed");
  Promise.reject(new Error("Unhandled rejection in afterAll"));
});