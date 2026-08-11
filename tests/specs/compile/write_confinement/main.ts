// A compiled app with no baked --allow-write gets the default write
// confinement: write is confined to the cwd and the OS temp directory, minus
// the `.git` directory inside the cwd.

// A real-fs file in the cwd is writable without --allow-write.
Deno.writeTextFileSync("./written.txt", "hi");
console.log("write cwd: ok");

// The `.git` directory inside the cwd is denied.
try {
  Deno.writeTextFileSync("./.git/config", "x");
  console.log("write .git: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const e = err as Error;
  const named = e.message.includes("--allow-write") ? "--allow-write" : "?";
  console.log(`write .git: ${e.name} ${named}`);
}

// A path outside the cwd and temp dir (passed as an argument) is denied.
const outside = Deno.args[0];
try {
  Deno.writeTextFileSync(outside + "/written_outside.txt", "x");
  console.log("write outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const e = err as Error;
  const named = e.message.includes("--allow-write") ? "--allow-write" : "?";
  console.log(`write outside: ${e.name} ${named}`);
}
