// Runs with -A so it can write to the OS temp directory (write is still
// deny-by-default and out of scope for this change). Creates a temp file and
// records its absolute path in the cwd for the confined run to read back.
const tempFile = Deno.makeTempFileSync();
Deno.writeTextFileSync(tempFile, "temp-data");
Deno.writeTextFileSync("./tempfile_path.txt", tempFile);
console.log("setup: ok");
