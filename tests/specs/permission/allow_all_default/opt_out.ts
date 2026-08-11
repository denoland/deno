// Providing any explicit allow-side flag (here --allow-read) disables the
// allow-all default entirely, so writing now requires --allow-write and is
// denied non-interactively.
try {
  Deno.writeTextFileSync("./should_fail.txt", "data");
  console.log("write: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-write") ? "--allow-write" : "?";
  console.log(`write: ${err.name} ${named}`);
}
