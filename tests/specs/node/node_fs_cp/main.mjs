import fs from "node:fs/promises";
import path from "node:path";

const src = path.join(import.meta.dirname, "source");
const target = path.join(import.meta.dirname, "dist");

try {
  await fs.rm(target, { recursive: true });
} catch {
  // ignore
}

await fs.cp(src, target, { recursive: true, force: true });
await fs.cp(src, target, { recursive: true, force: true });

const entries = await Array.fromAsync(await fs.readdir(target));
console.log(entries);

try {
  await fs.rm(target, { recursive: true });
} catch {
  // ignore
}
