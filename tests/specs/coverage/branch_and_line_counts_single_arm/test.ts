import { f } from "./mod.ts";

// Only the `if` arm runs. The `} else {` line carries the consequent's closing
// brace, which executed, so the line must count as covered even though the
// `else` arm never ran. Before the fix, a single reached edge of a line let an
// uncovered sibling range zero the whole line, so this junction counted as
// covered only when both arms ran in the same test process.
Deno.test("f", () => {
  f(true);
});
