// Each file performs a SubtleCrypto operation that takes a WebIDL `sequence`
// argument (`keyUsages`). See ../../../../tests/specs/test/
// crypto_webidl_sequence_cross_isolate/__test__.jsonc for context.
Deno.test("importKey with a keyUsages sequence", async () => {
  await crypto.subtle.importKey(
    "raw",
    new Uint8Array(16),
    { name: "AES-GCM" },
    false,
    ["encrypt"],
  );
});
