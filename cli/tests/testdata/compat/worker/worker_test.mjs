import { deferred } from "../../../../../test_util/std/async/deferred.ts";

const promise1 = deferred();
const promise2 = deferred();
const url = new URL("./worker.mjs", import.meta.url);
const worker = new Worker(url.href, { type: "module", deno: true });

worker.onmessage = (e) => {
    if (e.data == "hello from worker") {
        promise1.resolve();
        return;
    }

    const pid = e.data.pid;
    if (typeof pid != "number") {
        throw new Error("pid is not a number");
    }
    console.log("process.pid from worker:", pid);
    promise2.resolve();
};

await promise1;
worker.postMessage("hello");
await promise2;
worker.terminate();
