Deno.test("run hello", () => {
  console.log("hello from console.log");
  console.error("hello from console.error");
});

Deno.test("run boom", () => {
  throw new Error("boom!");
});

Deno.test("run ignored", { ignore: true }, () => {
});

Deno.test("filtered", () => {
});
