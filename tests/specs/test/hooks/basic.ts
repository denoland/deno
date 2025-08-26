Deno.test.beforeAll(() => {
  console.log("beforeAll 1");
});

Deno.test.beforeAll(() => {
  console.log("beforeAll 2");
});

Deno.test.beforeAll(() => {
  console.log("beforeAll 3");
});

// Multiple beforeEach hooks
Deno.test.beforeEach(() => {
  console.log("beforeEach 1");
});

Deno.test.beforeEach(() => {
  console.log("beforeEach 2");
});

// Multiple afterEach hooks
Deno.test.afterEach(() => {
  console.log("afterEach 1");
});

Deno.test.afterEach(() => {
  console.log("afterEach 2");
});

// Multiple afterAll hooks
Deno.test.afterAll(() => {
  console.log("afterAll 1");
});

Deno.test.afterAll(() => {
  console.log("afterAll 2");
});

Deno.test("first test", () => {
  console.log("test 1");
});

Deno.test("second test", () => {
  console.log("test 2");
});

Deno.test("third test", () => {
  console.log("test 3");
});
