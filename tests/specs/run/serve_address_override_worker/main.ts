const server = Deno.serve({
  port: 9004, // overridden to 9003 by DENO_SERVE_ADDRESS
  onListen({ hostname, port }) {
    console.log(`Main listening on ${hostname}:${port}`);
  },
}, () => new Response("main"));

const worker = new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
});
const workerPort = await new Promise((resolve) => {
  worker.onmessage = (e) => resolve(e.data);
});
console.log(`Worker listening on ${workerPort}`);

console.log(`Main response: ${await (await fetch("http://127.0.0.1:9003/")).text()}`);
console.log(`Worker response: ${await (await fetch(`http://127.0.0.1:${workerPort}/`)).text()}`);

worker.terminate();
await server.shutdown();
