// deno-lint-ignore-file

Deno.test("name", async () => {
  // console.log("hello world");
});

Deno.test("boom", async () => {
  throw new Error("boom boom!");
});

Deno.test("boom2", async () => {
  throw new Error("boom boom!");
});

Deno.test("boom3", async () => {
  throw new Error("boom boom!");
});
