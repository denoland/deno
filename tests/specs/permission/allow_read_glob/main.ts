// Tests glob entries in --allow-read. The allow list grants `sub/*.txt`, which
// matches only direct `.txt` children of `sub/`. We classify each access by
// whether the permission layer rejects it (NotCapable) before touching the
// filesystem. Allowed accesses instead fail with NotFound against the
// (nonexistent) file, which is immediate and offline-safe.

function isPermissionError(e: unknown): boolean {
  return e instanceof Deno.errors.NotCapable ||
    (e instanceof Error && e.name === "PermissionDenied");
}

function classifyRead(path: string): string {
  try {
    Deno.readTextFileSync(path);
    return "allowed";
  } catch (e) {
    return isPermissionError(e) ? "denied" : "allowed";
  }
}

// Matching direct child -> allowed.
console.log("sub/a.txt:", classifyRead("sub/a.txt"));
// `*` does not cross directory boundaries -> denied.
console.log("sub/nested/a.txt:", classifyRead("sub/nested/a.txt"));
// Wrong extension -> denied.
console.log("sub/a.log:", classifyRead("sub/a.log"));
// Outside the pattern's base directory -> denied.
console.log("other/a.txt:", classifyRead("other/a.txt"));
