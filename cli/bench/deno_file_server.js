// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
console.log("Server listening on", addr);
const tmpFile = await Deno.makeTempFile();
const file = await Deno.open(tmpFile, { write: true, read: true });
// 5MB
await file.write(new Uint8Array(1024 * 1024 * 5).fill(0));
file.close();

for await (const conn of listener) {
  handleHttp(conn);
}

async function handleHttp(conn) {
  const httpConn = Deno.serveHttp(conn);
  for await (const requestEvent of httpConn) {
    const fd = await Deno.open(tmpFile, { read: true });
    const response = new Response(fd.readable);
    await requestEvent.respondWith(response);
  }
}
