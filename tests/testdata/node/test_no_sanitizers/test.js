import test from "node:test";

test("should not complain about resource and op sanitizers", async (t) => {
  // resource
  const _file1 = Deno.open("welcome.ts");

  await t.test("nested test", () => {
    // resource
    const _file2 = Deno.open("cat.ts");

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

test("should allow exit", () => {
  // no exit sanitizers
  Deno.exit(123);
});
