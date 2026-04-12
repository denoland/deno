Deno.test.sanitizer({ resources: true });

Deno.test("resource leak caught by module sanitizer", () => {
  Deno.openSync("module_resources.ts");
});
