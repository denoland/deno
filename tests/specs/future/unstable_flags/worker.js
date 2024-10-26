import { delay } from "@std/async/delay";

const worker = new Worker(import.meta.resolve("./main.js"), { type: "module" });
await delay(1_000);
worker.terminate();
