// `Deno.exit(0)` at module top level should terminate this isolate cleanly
// (exit code 0) without killing the deno test process when there are other
// specifiers, and without leaving the unregistered test as a phantom result.
Deno.exit(0);

Deno.test("never registers", () => {
  throw new Error("should not run");
});
