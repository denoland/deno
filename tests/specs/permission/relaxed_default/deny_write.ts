// --deny-write=sub composes on top of the profile: writes under the denied
// subdir fail, while writes elsewhere in the cwd still succeed.
try {
  Deno.writeTextFileSync("./sub/denied.txt", "data");
  console.log("write sub: UNEXPECTEDLY ALLOWED");
} catch (err) {
  console.log(`write sub: ${err.name}`);
}
Deno.writeTextFileSync("./allowed.txt", "data");
console.log("write cwd: ok");
