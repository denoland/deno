// This member has no read grant: member_a's grant must not leak into the
// worker that evaluates this file, so this throws PermissionDenied.
Deno.readTextFileSync(import.meta.dirname + "/../member_a/data.txt");

Deno.test("other", () => {});
