// https://github.com/denoland/deno/issues/13729
// https://github.com/denoland/deno/issues/13938

Deno.test("test 1", () => {
  new Worker(`data:,close();`, {
    type: "module",
  });
});

