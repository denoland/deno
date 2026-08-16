// With the gate off (DENO_UNSTABLE_RELAXED_PERMISSIONS=0), behavior is
// byte-for-byte today's deny-by-default: reading outside the cwd is denied
// non-interactively naming --allow-read.
const path = Deno.build.os === "windows"
  ? "C:\\Windows\\win.ini"
  : "/etc/hosts";
try {
  Deno.readFileSync(path);
  console.log("read: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-read") ? "--allow-read" : "?";
  console.log(`read: ${err.name} ${named}`);
}
