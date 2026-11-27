// Top-level side effect: must happen exactly once for a file whose tests are
// all filtered out (it is evaluated only during the collect phase).
let existing = "";
try {
  existing = Deno.readTextFileSync("top_level_marker.txt");
} catch {
  // first run
}
Deno.writeTextFileSync("top_level_marker.txt", existing + "x");

Deno.test("other", () => {
  // The test body must never run under `--filter match`.
  Deno.writeTextFileSync("body_ran.txt", "yes");
});
