// Default write confinement (no --allow-write): write is confined to the cwd
// and the OS temp directory, minus the `.git` directory inside the cwd.

// A file inside the cwd is writable.
Deno.writeTextFileSync("./out.txt", "data");
console.log("write cwd: ok");

// A file inside a cwd subdirectory is writable.
Deno.mkdirSync("./sub", { recursive: true });
Deno.writeTextFileSync("./sub/nested.txt", "data");
console.log("write cwd subdir: ok");

// A file inside the OS temp directory is writable.
const tempFile = Deno.makeTempFileSync();
Deno.writeTextFileSync(tempFile, "temp-data");
console.log("write temp: ok");

// A write into `.git/hooks/pre-commit` inside the cwd is denied, naming
// --allow-write. This blocks planting a hook that runs on the next git command.
try {
  Deno.writeTextFileSync("./.git/hooks/pre-commit", "x");
  console.log("write .git hook: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = (err as Error).message.includes("--allow-write")
    ? "--allow-write"
    : "?";
  console.log(`write .git hook: ${(err as Error).name} ${named}`);
}

// A write into `.git/config` inside the cwd is denied too.
try {
  Deno.writeTextFileSync("./.git/config", "x");
  console.log("write .git config: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = (err as Error).message.includes("--allow-write")
    ? "--allow-write"
    : "?";
  console.log(`write .git config: ${(err as Error).name} ${named}`);
}

// A write outside the cwd and the temp directory (next to the deno executable)
// is denied, naming --allow-write.
try {
  Deno.writeTextFileSync(Deno.execPath() + ".written", "x");
  console.log("write outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = (err as Error).message.includes("--allow-write")
    ? "--allow-write"
    : "?";
  console.log(`write outside: ${(err as Error).name} ${named}`);
}
