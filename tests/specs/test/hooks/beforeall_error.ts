const logs: string[] = [];

Deno.test.beforeAll(() => {
  logs.push("beforeAll 1 executed");
  throw new Error("beforeAll 1 failed");
});

Deno.test.beforeAll(() => {
  logs.push("beforeAll 2 executed");
});

Deno.test("first test", () => {
  logs.push("test executed");
});

Deno.test.afterAll(() => {
  logs.push("afterAll 2 executed");
});

Deno.test.afterAll(() => {
  logs.push("afterAll 1 executed");
});

globalThis.onunload = () => {
  console.log(logs);
};
