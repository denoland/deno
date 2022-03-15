import { serve } from "https://deno.land/std/http/server.ts";
import { assert } from "https://deno.land/std/testing/asserts.ts";

async function client() {
  let tcpConn = await Deno.connect({ port: 4501 });
  tcpConn.write(
    new TextEncoder().encode(
      "CONNECT server.example.com:80 HTTP/1.1\r\n\r\nBla bla bla\nbla bla\nbla\n",
    ),
  );
  setTimeout(() => {
    tcpConn.write(
      new TextEncoder().encode(
        "Bla bla bla\nbla bla\nbla\n",
      ),
    );
  }, 500);
}

const server = serve((req) => {
  let p = Deno.upgradeHttp(req);

  (async () => {
    let [conn, firstPacket] = await p;
    const buf = new Uint8Array(1024);
    console.log(new TextDecoder().decode(firstPacket));
    const n = await conn.read(buf);
    assert(n != null);
    console.log(buf.slice(0, n));
  })();

  // this would hang forever. don't do the following:
  // await p;

  return new Response("", { status: 101 });
}, { port: 4501 });

await Promise.all([server, client()]);
