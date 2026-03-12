const logs: string[] = [];

Deno.test.beforeEach(() => {
  logs.push("beforeEach");
});

Deno.test.afterEach(() => {
  logs.push("afterEach");
});

Deno.test("first test", () => {
  logs.push("test executed");
  throw new Error("test threw");
});

globalThis.onunload = () => console.log(logs);
