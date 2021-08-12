import { copy } from "../../../test_util/std/io/util.ts";
const addr = Deno.args[0] || "0.0.0.0:4544";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
console.log("listening on", addr);
listener.accept().then(
  async (conn) => {
    console.log("received bytes:", await copy(conn, conn));
    conn.close();
    listener.close();
  },
);
