import { assert, assertEquals } from "@std/assert";
import { toText } from "@std/streams/to-text";

const encoder = new TextEncoder();

const { promise, resolve } = Promise.withResolvers();
const hostname = "localhost";
const port = 3505;

const listener = Deno.listenTls({
  hostname,
  port,
  cert: Deno.readTextFileSync("./tls/localhost.crt"),
  key: Deno.readTextFileSync("./tls/localhost.key"),
});

const response = encoder.encode(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
);

listener.accept().then(
  async (conn) => {
    assert(conn.remoteAddr != null);
    assert(conn.localAddr != null);
    await conn.write(response);
    // TODO(bartlomieju): this might be a bug
    setTimeout(() => {
      conn.close();
      resolve();
    }, 0);
  },
);

const conn = await Deno.connectTls({
  hostname,
  port,
});
await conn.writable.getWriter().write(
  encoder.encode(`GET / HTTP/1.1\r\nHost: ${hostname}:${port}\r\n\r\n`),
);
assertEquals(
  await toText(conn.readable),
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
);
listener.close();
await promise;

console.log("DONE");
