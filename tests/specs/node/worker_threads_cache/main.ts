import fs from "node:fs/promises";
import { isMainThread, Worker } from "node:worker_threads";

await fs.writeFile("mod.mjs", "export default " + isMainThread);

const path = new URL("mod.mjs", import.meta.url);
const i = await import(path.href);
console.log(i);

if (isMainThread) {
  const worker = new Worker(new URL("main.ts", import.meta.url));
  worker.on("message", (msg) => console.log(msg));
}
