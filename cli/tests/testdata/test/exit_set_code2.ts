Deno.test("ok", function () {
  // pass
});

Deno.test("error", function () {
  throw new Error("boom!");
});

self.onunload = () => {
  Deno.exit(0);
};
