const logs: string[] = [];

Deno.describe("suite", () => {
  Deno.test("skipped test", () => {
    logs.push("should not run");
  });

  Deno.test.only("focused test", () => {
    logs.push("focused");
  });
});

globalThis.onunload = () => {
  console.log(JSON.stringify(logs));
};
