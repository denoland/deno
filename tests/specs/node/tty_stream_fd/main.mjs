import process from "node:process";
import fs from "node:fs";

// process.stdout.fd and process.stderr.fd should always be set,
// regardless of whether the stream is a TTY or not.
// This is needed by libraries like @google/gemini-cli that use
// fs.writeSync(process.stdout.fd, data) for terminal capability detection.
console.log("stdout.fd:", typeof process.stdout.fd, process.stdout.fd);
console.log("stderr.fd:", typeof process.stderr.fd, process.stderr.fd);

// fs.writeSync with process.stdout.fd should work
fs.writeSync(process.stdout.fd, "writeSync works\n");
