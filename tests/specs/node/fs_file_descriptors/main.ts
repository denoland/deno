import { closeSync, fstatSync, openSync, readSync, writeSync } from "node:fs";
import { Buffer } from "node:buffer";

const tempFile = Deno.makeTempFileSync();

// Test that openSync returns a real OS FD (should be >= 3, since 0-2 are stdio)
const fd = openSync(tempFile, "w+");
console.log(`fd >= 3: ${fd >= 3}`);

// Test that writeSync works with the FD
const written = writeSync(fd, Buffer.from("hello fd"));
console.log(`bytes written: ${written}`);

// Test that fstatSync works with the FD
const stat = fstatSync(fd);
console.log(`stat.size: ${stat.size}`);

// Test that readSync works with the FD (seek to beginning first)
const buf = Buffer.alloc(8);
const bytesRead = readSync(fd, buf, 0, 8, 0);
console.log(`bytes read: ${bytesRead}`);
console.log(`data: ${buf.toString()}`);

// Test that closeSync works with the FD
closeSync(fd);
console.log("closed successfully");

Deno.removeSync(tempFile);
