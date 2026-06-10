// Regression test for https://github.com/denoland/deno/issues/34931
//
// A serve handler that returns a streaming `Response` backed by a resource
// that has already been closed (here a `using` file handle that is disposed
// the moment the handler returns) used to take the whole server process down
// with an uncaught `BadResource` rejection escaping from
// `fastSyncResponseOrStream`. It must instead respond with a 500 and keep the
// server alive for subsequent requests.

const tempFile = Deno.makeTempFileSync();
Deno.writeTextFileSync(tempFile, "hello from a soon-to-be-closed file");

const ac = new AbortController();
const server = Deno.serve(
  { port: 0, signal: ac.signal, onListen() {} },
  async (_req) => {
    using file = await Deno.open(tempFile);
    return new Response(file.readable);
  },
);

const port = (server.addr as Deno.NetAddr).port;

const res1 = await fetch(`http://localhost:${port}/`);
await res1.body?.cancel();
console.log("first status:", res1.status);

// The server must still be alive to serve a second request.
const res2 = await fetch(`http://localhost:${port}/`);
await res2.body?.cancel();
console.log("second status:", res2.status);

ac.abort();
await server.finished;
Deno.removeSync(tempFile);
console.log("server shut down cleanly");
