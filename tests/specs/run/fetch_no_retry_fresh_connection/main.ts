// Regression test for https://github.com/denoland/deno/issues/35610
// A server that accepts a request and then resets the connection must see
// the request exactly once - a failure on a freshly established connection
// is not safe to retry, since the server may have already processed it.

const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
const port = (listener.addr as Deno.NetAddr).port;

let received = 0;
const server = (async () => {
  for await (const conn of listener) {
    const buf = new Uint8Array(1024);
    await conn.read(buf);
    received++;
    conn.close();
  }
})();

try {
  await fetch(`http://127.0.0.1:${port}/`);
  console.log("fetch unexpectedly succeeded");
} catch (err) {
  console.log("fetch failed:", err instanceof TypeError);
}

listener.close();
await server.catch(() => {});
console.log("server received requests:", received);
