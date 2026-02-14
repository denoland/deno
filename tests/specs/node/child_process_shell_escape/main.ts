// Test that shell metacharacters in arguments are properly escaped
// when using shell: true with spawn/spawnSync to prevent command injection.

import { spawn, spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";

const tempDir = Deno.cwd();
const markerFile = path.join(tempDir, "injection_marker");

// Clean up any existing marker file
try {
  fs.unlinkSync(markerFile);
} catch {
  // ignore
}

// Test 1: Newline injection should be blocked
console.log("Test 1: Newline injection in args");
const newlinePayload = `dummy\ntouch ${markerFile}`;
spawnSync("echo", [newlinePayload], { shell: true });
if (fs.existsSync(markerFile)) {
  console.log("FAIL: Newline injection was not blocked");
  Deno.exit(1);
} else {
  console.log("PASS: Newline injection blocked");
}

// Test 2: Semicolon injection should be blocked
console.log("Test 2: Semicolon injection in args");
const semicolonPayload = `dummy; touch ${markerFile}`;
spawnSync("echo", [semicolonPayload], { shell: true });
if (fs.existsSync(markerFile)) {
  console.log("FAIL: Semicolon injection was not blocked");
  Deno.exit(1);
} else {
  console.log("PASS: Semicolon injection blocked");
}

// Test 3: Pipe injection should be blocked
console.log("Test 3: Pipe injection in args");
const pipePayload = `dummy | touch ${markerFile}`;
spawnSync("echo", [pipePayload], { shell: true });
if (fs.existsSync(markerFile)) {
  console.log("FAIL: Pipe injection was not blocked");
  Deno.exit(1);
} else {
  console.log("PASS: Pipe injection blocked");
}

// Test 4: Backtick injection should be blocked
console.log("Test 4: Backtick injection in args");
const backtickPayload = "`touch " + markerFile + "`";
spawnSync("echo", [backtickPayload], { shell: true });
if (fs.existsSync(markerFile)) {
  console.log("FAIL: Backtick injection was not blocked");
  Deno.exit(1);
} else {
  console.log("PASS: Backtick injection blocked");
}

// Test 5: $() injection should be blocked
console.log("Test 5: $() injection in args");
const dollarPayload = "$(touch " + markerFile + ")";
spawnSync("echo", [dollarPayload], { shell: true });
if (fs.existsSync(markerFile)) {
  console.log("FAIL: $() injection was not blocked");
  Deno.exit(1);
} else {
  console.log("PASS: $() injection blocked");
}

// Test 6: Normal functionality still works - args are passed correctly
console.log("Test 6: Normal args work correctly");
const result = spawnSync("echo", ["hello", "world"], {
  shell: true,
  encoding: "utf-8",
});
if (result.stdout?.trim() === "hello world") {
  console.log("PASS: Normal args work");
} else {
  console.log("FAIL: Normal args broken, got:", result.stdout?.trim());
  Deno.exit(1);
}

// Test 7: Args with spaces are preserved
console.log("Test 7: Args with spaces preserved");
const result2 = spawnSync("echo", ["hello world"], {
  shell: true,
  encoding: "utf-8",
});
if (result2.stdout?.trim() === "hello world") {
  console.log("PASS: Args with spaces work");
} else {
  console.log("FAIL: Args with spaces broken, got:", result2.stdout?.trim());
  Deno.exit(1);
}

// Test 8: Shell features work when using string command (no args)
console.log("Test 8: Shell features work with string command");
const result3 = spawnSync("echo foo | cat", { shell: true, encoding: "utf-8" });
if (result3.stdout?.trim() === "foo") {
  console.log("PASS: Shell features work");
} else {
  console.log("FAIL: Shell features broken, got:", result3.stdout?.trim());
  Deno.exit(1);
}

// Test 9: Async spawn also escapes args
console.log("Test 9: Async spawn escapes args");
await new Promise<void>((resolve) => {
  const child = spawn("echo", [`dummy; touch ${markerFile}`], { shell: true });
  child.on("close", () => {
    if (fs.existsSync(markerFile)) {
      console.log("FAIL: Async spawn injection was not blocked");
      Deno.exit(1);
    } else {
      console.log("PASS: Async spawn injection blocked");
    }
    resolve();
  });
});

console.log("All tests passed!");
