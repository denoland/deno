import { serve } from "https://deno.land/std/http/server.ts";

const x = serve((req) => {
  let p = Deno.upgradeHttp(req);

  (async () => {
    let [conn, firstPacket] = await p;
    const buf = new Uint8Array(1024);
    console.log(new TextDecoder().decode(firstPacket));
    const n = await conn.read(buf);
    console.log(buf.slice(0, n));
  })();

  // this would hang forever. don't do the following:
  // await p;

  return new Response({ status: 101 });
}, { port: 8080 });

async function client() {
  let tcp_conn = await Deno.connect({ host: "127.0.0.1", port: 8080 });
  tcp_conn.write(
    new TextEncoder().encode(
      "CONNECT server.example.com:80 HTTP/1.1\r\n\r\nBla bla bla\nbla bla\nbla\n",
    ),
  );
  setTimeout(() => {
    tcp_conn.write(
      new TextEncoder().encode(
        "Bla bla bla\nbla bla\nbla\n",
      ),
    );
  }, 500);
}

await Promise.all([x, client()]);
