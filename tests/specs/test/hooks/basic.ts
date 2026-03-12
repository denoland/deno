const logs: string[] = [];

Deno.test.beforeAll(() => {
  logs.push("beforeAll 1");
});

Deno.test.beforeAll(() => {
  logs.push("beforeAll 2");
});

Deno.test.beforeAll(() => {
  logs.push("beforeAll 3");
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

Deno.test("first test", () => {
  logs.push("test 1");
});

Deno.test("second test", () => {
  logs.push("test 2");
});

Deno.test("third test", () => {
  logs.push("test 3");
});

globalThis.onunload = () => {
  console.log(logs);
};
