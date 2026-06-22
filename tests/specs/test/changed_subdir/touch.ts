// Modify a source file in the subdirectory so it shows up as an unstaged
// change (the case `git ls-files` reports relative to the current directory).
Deno.writeTextFileSync(
  "sub/math.ts",
  Deno.readTextFileSync("sub/math.ts") + "\n// changed\n",
);
