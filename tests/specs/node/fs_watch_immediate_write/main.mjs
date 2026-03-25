import fs from "node:fs";
import path from "node:path";
import os from "node:os";

const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "watch-test-"));
const filePath = path.join(tmpDir, "test.txt");

fs.writeFileSync(filePath, "initial content");

const { promise, resolve } = Promise.withResolvers();

const watcher = fs.watch(filePath, (eventType, filename) => {
  console.log(`Event: ${eventType}`);
  watcher.close();
  resolve();
});

// Write immediately after setting up the watcher — this is the bug scenario.
fs.writeFileSync(filePath, "modified content");

const timeout = setTimeout(() => {
  console.log("Timeout: no event received");
  watcher.close();
  resolve();
}, 5000);

await promise;
clearTimeout(timeout);

// Cleanup
fs.rmSync(tmpDir, { recursive: true });
