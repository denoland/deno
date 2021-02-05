// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// Used for benchmarking Deno's tcp proxy performance.
const addr = Deno.args[0] || "127.0.0.1:4500";
const originAddr = Deno.args[1] || "127.0.0.1:4501";

const [hostname, port] = addr.split(":");
const [originHostname, originPort] = originAddr.split(":");

const listener = Deno.listen({ hostname, port: Number(port) });

async function handle(conn: Deno.Conn): Promise<void> {
  const origin = await Deno.connect({
    hostname: originHostname,
    port: Number(originPort),
  });
  try {
    await Promise.all([Deno.copy(conn, origin), Deno.copy(origin, conn)]);
  } catch (err) {
    if (err.message !== "read error" && err.message !== "write error") {
      throw err;
    }
  } finally {
    conn.close();
    origin.close();
  }
}

console.log(`Proxy listening on http://${addr}/`);
for await (const conn of listener) {
  handle(conn);
}
