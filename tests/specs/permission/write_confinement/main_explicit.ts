// With an explicit --allow-write=allowed.txt, the default cwd+temp confinement
// is additive: cwd and temp are appended to the explicit grant, so another file
// in the cwd and a temp file are writable, while a path outside cwd+temp and the
// explicit grant still denies.

// The explicitly allowed file is writable.
Deno.writeTextFileSync("./allowed.txt", "data");
console.log("write allowed file: ok");

// Another file in the same cwd is writable too, because cwd was appended.
Deno.writeTextFileSync("./other.txt", "data");
console.log("write other cwd file: ok");

// A file inside the OS temp directory is writable, because temp was appended.
const tempFile = Deno.makeTempFileSync();
Deno.writeTextFileSync(tempFile, "temp-data");
console.log("write temp: ok");

// A file outside the cwd, the temp dir, and the explicit grant is still denied.
try {
  Deno.writeTextFileSync(Deno.execPath() + ".written", "x");
  console.log("write outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = (err as Error).message.includes("--allow-write")
    ? "--allow-write"
    : "?";
  console.log(`write outside: ${(err as Error).name} ${named}`);
}
