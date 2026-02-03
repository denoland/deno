// Copyright 2018-2026 the Deno authors. MIT license.
// Test: Can we read arbitrary fds via lazy dup?

import fs from "node:fs";
import path from "node:path";

const dir = import.meta.dirname!;
const secretFile = path.join(dir, "secret.txt");

Deno.writeTextFileSync(secretFile, "SECRET DATA!");

console.log("=== Lazy dup vulnerability test ===\n");

// Step 1: Open secret file via DENO API (not node:fs)
// This creates a Deno resource but does NOT register in node:fs fdMap
const denoHandle = Deno.openSync(secretFile, { read: true });
console.log(`1. Opened secret file via Deno.openSync()`);

// Step 2: Get the actual OS fd from the Deno handle
// We need to find what fd the kernel assigned
// Let's open via node:fs and see what fd we get (should be next available)
const probeFile = path.join(dir, "probe.txt");
Deno.writeTextFileSync(probeFile, "probe");
const probeFd = fs.openSync(probeFile, "r");
console.log(`2. Probe fd (next available): ${probeFd}`);
fs.closeSync(probeFd);
Deno.removeSync(probeFile);

// The Deno handle likely has fd = probeFd - 1 or nearby
// Let's try to read fd values around the probe
console.log(`3. Attempting to read arbitrary fds via node:fs...`);

for (let fd = 3; fd < probeFd + 5; fd++) {
  try {
    const buf = Buffer.alloc(50);
    const bytesRead = fs.readSync(fd, buf, 0, 50, 0);
    const content = buf.toString("utf8", 0, bytesRead).trim();
    console.log(`   fd ${fd}: READ SUCCESS "${content.substring(0, 30)}..."`);

    if (content.includes("SECRET")) {
      console.log(`\n!!! FOUND SECRET at fd ${fd} !!!`);
    }
  } catch (e) {
    // Silently skip failures
  }
}

denoHandle.close();
Deno.removeSync(secretFile);
