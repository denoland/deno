// The real config lives in a sibling `real/` dir and cwd `config.json` is a
// SYMLINK to it. The permission set grants write to `.` (relative to the config
// file's directory).
Deno.mkdirSync("real");
Deno.writeTextFileSync(
  "real/deno.json",
  JSON.stringify({ permissions: { write: { write: ["."] } } }),
);
// Store the target with the platform separator so it resolves on Windows too.
const sep = Deno.build.os === "windows" ? "\\" : "/";
Deno.symlinkSync(`real${sep}deno.json`, "config.json");
