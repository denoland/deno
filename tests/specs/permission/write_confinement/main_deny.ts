// --deny-write=sub composes on top of the default cwd+temp grant, and the
// `.git` deny is merged with the user --deny-write rather than overwriting it.

// The cwd is still writable.
Deno.writeTextFileSync("./out.txt", "data");
console.log("write cwd: ok");

// The denied subdirectory is not (the user --deny-write wins over the cwd
// grant).
try {
  Deno.writeTextFileSync("./sub/nested.txt", "x");
  console.log("write denied subdir: UNEXPECTEDLY ALLOWED");
} catch (err) {
  console.log(`write denied subdir: ${(err as Error).name}`);
}

// The `.git` deny is still in effect alongside the user --deny-write (proving
// the merge, not a clobber).
try {
  Deno.writeTextFileSync("./.git/config", "x");
  console.log("write .git config: UNEXPECTEDLY ALLOWED");
} catch (err) {
  console.log(`write .git config: ${(err as Error).name}`);
}
