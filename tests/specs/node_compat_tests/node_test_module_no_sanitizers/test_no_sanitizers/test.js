import test from "node:test";
test("should not complain about resource and op sanitizers", async (t) => {
  // resource
  const _file1 = Deno.open("test_no_sanitizers/welcome.ts");

  await t.test("nested test", () => {
    // resource
    const _file2 = Deno.open("test_no_sanitizers/cat.ts");

    // op
    crypto.subtle.digest(
      "SHA-256",
      new TextEncoder().encode("a".repeat(1_000_000)),
    );
  });

  // op
  crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode("a".repeat(1_000_000)),
  );
});

// TODO(mmastrac): This works, but we don't reliably flush stdout/stderr here, making this test flake
// test("should allow exit", () => {
//   // no exit sanitizers
//   Deno.exit(123);
// });
