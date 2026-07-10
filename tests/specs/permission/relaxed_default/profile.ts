// Relaxed default permission profile (gate on, no flags): read is allowed
// everywhere, write is allowed under the cwd and the OS temp directory, and
// env is allowed. net stays gated (see net.ts).

// read a file outside the cwd (the deno executable)
Deno.readFileSync(Deno.execPath());
console.log("read outside cwd: ok");

// write inside the cwd
Deno.writeTextFileSync("./in_cwd.txt", "data");
console.log("write cwd: ok");

// write inside the OS temp directory
const tempFile = Deno.makeTempFileSync();
Deno.writeTextFileSync(tempFile, "data");
console.log("write temp: ok");

// write outside both the cwd and the temp directory is denied
const outside = Deno.build.os === "windows"
  ? "C:\\relaxed_denied.txt"
  : "/relaxed_denied.txt";
try {
  Deno.writeTextFileSync(outside, "data");
  console.log("write outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-write") ? "--allow-write" : "?";
  console.log(`write outside: ${err.name} ${named}`);
}

// env access is allowed for the curated default-readable allowlist
console.log(`env: ${Deno.env.get("TERM")}`);
