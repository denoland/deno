// Providing any explicit allow-side flag (here --allow-net) disables the
// relaxed profile entirely, so writing to the cwd now requires --allow-write
// and is denied.
try {
  Deno.writeTextFileSync("./should_fail.txt", "data");
  console.log("write cwd: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-write") ? "--allow-write" : "?";
  console.log(`write cwd: ${err.name} ${named}`);
}
