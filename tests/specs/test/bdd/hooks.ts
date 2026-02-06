const logs: string[] = [];

Deno.describe("suite", () => {
  Deno.beforeAll(() => {
    logs.push("beforeAll");
  });

  Deno.afterAll(() => {
    logs.push("afterAll");
  });

  Deno.beforeEach(() => {
    logs.push("beforeEach");
  });

  Deno.afterEach(() => {
    logs.push("afterEach");
  });

  Deno.test("test 1", () => {
    logs.push("test 1");
  });

  Deno.test("test 2", () => {
    logs.push("test 2");
  });

  Deno.describe("nested", () => {
    Deno.beforeEach(() => {
      logs.push("nested beforeEach");
    });

    Deno.afterEach(() => {
      logs.push("nested afterEach");
    });

    Deno.test("test 3", () => {
      logs.push("test 3");
    });
  });
});

globalThis.onunload = () => {
  console.log(JSON.stringify(logs));
};
