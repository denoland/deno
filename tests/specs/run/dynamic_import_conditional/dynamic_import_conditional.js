const Worker = globalThis.Worker ?? (await import("worker_threads")).Worker;

console.log(!!Worker);
