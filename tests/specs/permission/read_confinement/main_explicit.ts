// With an explicit --allow-read=inside.txt, the default cwd+temp confinement is
// additive: cwd and temp are appended to the explicit grant, so another file in
// the cwd is readable, while a path outside cwd+temp and the explicit grant
// still denies.

// The explicitly allowed file is readable.
Deno.readTextFileSync("./inside.txt");
console.log("read allowed file: ok");

// Another file in the same cwd is readable too, because cwd was appended.
Deno.readTextFileSync("./other.txt");
console.log("read other cwd file: ok");

// A file outside the cwd, the temp dir, and the explicit grant is still denied.
try {
  Deno.readFileSync(Deno.execPath());
  console.log("read outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-read") ? "--allow-read" : "?";
  console.log(`read outside: ${err.name} ${named}`);
}
