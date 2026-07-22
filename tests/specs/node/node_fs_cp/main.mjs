import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";

const src = path.join(import.meta.dirname, "source");
const target = path.join(import.meta.dirname, "dist");

try {
  await fs.rm(target, { recursive: true });
} catch {
  // ignore
}

assert.strictEqual(
  await fs.cp(src, target, { recursive: true, force: true }),
  undefined,
);
assert.strictEqual(
  await fs.cp(src, target, { recursive: true, force: true }),
  undefined,
);

const entries = await Array.fromAsync(await fs.readdir(target));
console.log(entries);

try {
  await fs.rm(target, { recursive: true });
} catch {
  // ignore
}
