// A compiled app with no baked --allow-read gets the default read confinement:
// read is confined to the cwd and the OS temp directory. The embedded VFS files
// are granted separately.

// A real-fs file in the cwd is readable without --allow-read.
Deno.readTextFileSync("./cwd_file.txt");
console.log("read cwd: ok");

// A path outside the cwd and temp dir (passed as an argument) is denied.
const outside = Deno.args[0];
try {
  Deno.readDirSync(outside)[Symbol.iterator]().next();
  console.log("read outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const e = err as Error;
  const named = e.message.includes("--allow-read") ? "--allow-read" : "?";
  console.log(`read outside: ${e.name} ${named}`);
}
