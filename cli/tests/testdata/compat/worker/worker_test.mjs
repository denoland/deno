import { deferred } from "../../../../../test_util/std/async/deferred.ts";

const promise = deferred();
const url = new URL("./worker.mjs", import.meta.url);
const worker = new Worker(url.href, { type: "module", deno: true });

worker.onmessage = (e) => {
    const pid = e.data.pid;
    if (typeof pid != "number") {
        throw new Error("pid is not a number");
    }
    console.log("process.pid from worker:", pid);
    promise.resolve();
};

worker.postMessage("hello");
await promise;
worker.terminate();
