// --deny-read=sub composes on top of the default cwd+temp grant.

// The cwd is still readable.
Deno.readTextFileSync("./inside.txt");
console.log("read cwd: ok");

// The denied subdirectory is not (deny wins over the cwd grant).
try {
  Deno.readTextFileSync("./sub/nested.txt");
  console.log("read denied subdir: UNEXPECTEDLY ALLOWED");
} catch (err) {
  console.log(`read denied subdir: ${err.name}`);
}
