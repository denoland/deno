// Copyright 2018-2026 the Deno authors. MIT license.
// Repro for fd/rid reuse attack
// Testing various ways to close fds that might bypass fs.close()

import fs from "node:fs";
import path from "node:path";

const dir = import.meta.dirname!;
const secretFile = path.join(dir, "secret.txt");
const publicFile = path.join(dir, "public.txt");

// Create test files
Deno.writeTextFileSync(secretFile, "SECRET DATA - should not be readable!");
Deno.writeTextFileSync(publicFile, "public data");

console.log("=== Testing attack vectors ===\n");

// Vector 1: Deno.close() - removed in Deno 2
console.log("1. Deno.close():", typeof Deno.close);

// Vector 2: Check if core.close is accessible
console.log("2. Deno.core:", typeof (Deno as any).core);

// Vector 3: Check globalThis for close
console.log("3. globalThis.close:", typeof (globalThis as any).close);

// Vector 4: Open via node:fs and try to close the same fd via Deno.open
console.log("\n=== Attempting fd confusion attack ===\n");

const fd = fs.openSync(publicFile, "r");
console.log(`4. Opened public file via fs.openSync(), got fd: ${fd}`);

// What if we open the same fd number via Deno?
// This shouldn't work because fd numbers come from the kernel
try {
  // Can't directly specify fd, but what if kernel reuses?
  fs.closeSync(fd); // This properly unregisters
  console.log(`5. Closed fd ${fd} via fs.closeSync() - properly unregistered`);

  // Now open secret file
  const secretFd = fs.openSync(secretFile, "r");
  console.log(`6. Opened secret file, got fd: ${secretFd}`);

  // Try to read with old fd - should fail because it's unregistered
  try {
    const buf = Buffer.alloc(100);
    fs.readSync(fd, buf);
    console.log("7. ERROR: Read succeeded on closed fd!");
  } catch (e) {
    console.log(`7. Read on old fd failed as expected: ${e.message}`);
  }

  fs.closeSync(secretFd);
} catch (e) {
  console.log(`Error: ${e.message}`);
}

// Cleanup
try { Deno.removeSync(secretFile); } catch {}
try { Deno.removeSync(publicFile); } catch {}

console.log("\n=== Conclusion ===");
console.log("Deno 2 removed Deno.close() and Deno.resources()");
console.log("Attack requires FFI (--allow-ffi) or other privileged access");
