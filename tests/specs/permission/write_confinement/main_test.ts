// The process runs under the default cwd+temp write confinement, but a test
// with permissions: { write: false } must fully deny write (no leak from the
// default into the child permissions).
Deno.test({
  name: "write false denies cwd",
  permissions: { write: false },
}, () => {
  try {
    Deno.writeTextFileSync("./leak.txt", "x");
    throw new Error("write was UNEXPECTEDLY ALLOWED");
  } catch (err) {
    if (!(err instanceof Deno.errors.NotCapable)) {
      throw err;
    }
  }
});
