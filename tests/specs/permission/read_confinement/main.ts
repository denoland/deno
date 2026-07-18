// Default read confinement (no --allow-read): read is confined to the cwd and
// the OS temp directory.

// A file inside the cwd is readable.
Deno.readTextFileSync("./inside.txt");
console.log("read cwd: ok");

// A file inside a cwd subdirectory is readable.
Deno.readTextFileSync("./sub/nested.txt");
console.log("read cwd subdir: ok");

// A file inside the OS temp directory (created by the -A setup run) is
// readable.
const tempPath = Deno.readTextFileSync("./tempfile_path.txt");
Deno.readTextFileSync(tempPath);
console.log("read temp: ok");

// A file outside the cwd and the temp directory (the deno executable) is
// denied, naming --allow-read.
try {
  Deno.readFileSync(Deno.execPath());
  console.log("read outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-read") ? "--allow-read" : "?";
  console.log(`read outside: ${err.name} ${named}`);
}
