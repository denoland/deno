import { delay } from "../../../util/std/async/delay.ts";

const worker = new Worker(import.meta.resolve("./main.js"), { type: "module" });
await delay(1_000);
worker.terminate();
