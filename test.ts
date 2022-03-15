import { serve } from "https://deno.land/std/http/server.ts";
import { assert, assertEquals } from "https://deno.land/std/testing/asserts.ts";

async function client() {
  let tcpConn = await Deno.connect({ port: 4501 });
  await tcpConn.write(
    new TextEncoder().encode(
      "CONNECT server.example.com:80 HTTP/1.1\r\n\r\nbla bla bla\nbla bla\nbla\n",
    ),
  );
  setTimeout(async () => {
    await tcpConn.write(
      new TextEncoder().encode(
        "bla bla bla\nbla bla\nbla\n",
      ),
    );
  }, 500);
}

const abortController = new AbortController();
const signal = abortController.signal;

const server = serve((req) => {
  let p = Deno.upgradeHttp(req);

  (async () => {
    let [conn, firstPacket] = await p;
    const buf = new Uint8Array(1024);
    const firstPacketText = new TextDecoder().decode(firstPacket);
    assertEquals(firstPacketText, "bla bla bla\nbla bla\nbla\n");
    const n = await conn.read(buf);
    assert(n != null);
    const secondPacketText = new TextDecoder().decode(buf.slice(0, n));
    assertEquals(secondPacketText, "bla bla bla\nbla bla\nbla\n");
    abortController.abort();
  })();

  return new Response({ status: 101 });
}, { port: 4501, signal });

await Promise.all([server, client()]);
