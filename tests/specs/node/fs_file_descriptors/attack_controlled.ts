// Copyright 2018-2026 the Deno authors. MIT license.
// Controlled attack: force fd reuse by closing fds first

import fs from "node:fs";
import path from "node:path";

const dir = import.meta.dirname!;
const secretFile = path.join(dir, "secret.txt");
const publicFile = path.join(dir, "public.txt");

Deno.writeTextFileSync(secretFile, "SECRET DATA!");
Deno.writeTextFileSync(publicFile, "public");

console.log("=== Controlled fd reuse attack ===\n");

// Step 1: Open public file
const fd1 = fs.openSync(publicFile, "r");
console.log(`1. Opened public file, got fd: ${fd1}`);

// Step 2: Properly close it (this unregisters from fdMap)
fs.closeSync(fd1);
console.log(`2. Closed fd ${fd1} properly via fs.closeSync()`);

// Step 3: Open secret file - should get same fd number
const fd2 = fs.openSync(secretFile, "r");
console.log(`3. Opened secret file, got fd: ${fd2}`);

// Step 4: Try to read using OLD fd number
// Since fd1 was properly closed and unregistered, getRid(fd1) should:
// - Not find fd1 in the map (it was unregistered)
// - Try lazy dup on fd1... but fd1 == fd2 now!
console.log(`4. Trying to read old fd ${fd1} (which now points to secret)...`);

if (fd1 === fd2) {
  console.log(`   Note: fd numbers are the same! fd1=${fd1}, fd2=${fd2}`);
}

try {
  const buf = Buffer.alloc(100);
  const bytesRead = fs.readSync(fd1, buf, 0, 100, 0);
  const content = buf.toString("utf8", 0, bytesRead).trim();
  console.log(`5. Read via old fd: "${content}"`);

  if (content.includes("SECRET") && fd1 !== fd2) {
    console.log("\n!!! ATTACK SUCCESSFUL - read secret via stale fd !!!");
  } else if (fd1 === fd2) {
    console.log("\n   (fd reused, but this is the same registered fd)");
  }
} catch (e) {
  console.log(`5. Read failed: ${e.message}`);
}

fs.closeSync(fd2);
Deno.removeSync(secretFile);
Deno.removeSync(publicFile);
