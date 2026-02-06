const logs: string[] = [];

Deno.describe("suite", () => {
  Deno.it("skipped test", () => {
    logs.push("should not run");
  });

  Deno.it.only("focused test", () => {
    logs.push("focused");
  });
});

globalThis.onunload = () => {
  console.log(JSON.stringify(logs));
};
