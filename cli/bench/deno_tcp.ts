// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Used for benchmarking Deno's networking.
// TODO Replace this with a real HTTP server once
// https://github.com/denoland/deno/issues/726 is completed.
// Note: this is a keep-alive server.
const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
const response = new TextEncoder().encode(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
);
async function handle(conn: Deno.Conn): Promise<void> {
  const buffer = new Uint8Array(1024);
  try {
    while (true) {
      const r = await conn.read(buffer);
      if (r === null) {
        break;
      }
      await conn.write(response);
    }
  } finally {
    conn.close();
  }
}

console.log("Listening on", addr);
for await (const conn of listener) {
  handle(conn);
}
