function assert(condition, message) {
  if (!condition) throw new Error(message);
}

Deno.test("uses the version from the import map", () => {
  assert(
    document.mockDomLibrary === "happy-dom",
    "document should come from happy-dom",
  );
  assert(
    document.version === "1.0.0",
    "expected the import map version, got " + document.version,
  );
});
