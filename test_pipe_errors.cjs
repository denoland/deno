// Debug script for Windows pipe connect errors
// Run with: ./target/debug/deno run -A test_pipe_errors.cjs
const net = require("net");

console.log("test 1: connect to non-existent path");
const c1 = net.createConnection("no-ent-file");
c1.on("error", (e) => console.log("  error:", e.code, e.message));
c1.on("connect", () => console.log("  connected (unexpected!)"));

console.log("test 2: connect to regular file (package.json)");
const c2 = net.createConnection("package.json");
c2.on("error", (e) => console.log("  error:", e.code, e.message));
c2.on("connect", () => console.log("  connected (unexpected!)"));

console.log("test 3: connect to named pipe path that doesn't exist");
const c3 = net.createConnection("\\\\.\\pipe\\nonexistent-test-pipe");
c3.on("error", (e) => console.log("  error:", e.code, e.message));
c3.on("connect", () => console.log("  connected (unexpected!)"));

setTimeout(() => {
  console.log("TIMEOUT - one of the connects is hanging");
  process.exit(1);
}, 5000);
