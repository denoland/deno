// Relaxed default permission profile (gate on, no flags): read and write are
// both confined to the cwd and the OS temp directory, and env is allowed. net
// stays gated (see net.ts).

// read a file inside the cwd
Deno.writeTextFileSync("./readable.txt", "data");
Deno.readTextFileSync("./readable.txt");
console.log("read cwd: ok");

// read a file inside the OS temp directory
const tempRead = Deno.makeTempFileSync();
Deno.readTextFileSync(tempRead);
console.log("read temp: ok");

// read a file outside the cwd and the temp directory (the deno executable) is
// denied
try {
  Deno.readFileSync(Deno.execPath());
  console.log("read outside: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-read") ? "--allow-read" : "?";
  console.log(`read outside: ${err.name} ${named}`);
}

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
