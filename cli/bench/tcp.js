// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

const listener = Deno.listen({ port: 4500 });
const response = new TextEncoder().encode(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
);

// Accept a connection and write packets as fast as possible.
async function acceptWrite() {
  const conn = await listener.accept();
  try {
    while (true) {
      await conn.write(response);
    }
  } catch {
    // Pass
  }
  conn.close();
}

await acceptWrite();
await acceptWrite();
