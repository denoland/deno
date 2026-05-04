// --timeout=100 sets a specifier-wide default; the test below has no
// per-test override so it inherits 100ms and times out.
Deno.test("inherits the cli default", async () => {
  await new Promise((resolve) => setTimeout(resolve, 1000));
});
