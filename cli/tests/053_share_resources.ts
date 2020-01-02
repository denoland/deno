const _f = Deno.openSync("./shared_resource_worker.ts");
console.log(Deno.resources());

const w0 = new Worker("./shared_resource_worker.ts", {
  shareResources: true
});
w0.postMessage(0);
await w0.closed;

const w1 = new Worker("./shared_resource_worker.ts");
w1.postMessage(1);
await w1.closed;
