import { assert, assertEquals } from "@std/assert";
import { BufReader } from "@std/io/buf-reader";
import { BufWriter } from "@std/io/buf-writer";
import { TextProtoReader } from "./textproto.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

const { promise, resolve } = Promise.withResolvers();
const hostname = "localhost";
const port = 3504;

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

let conn = await Deno.connect({ hostname, port });
conn = await Deno.startTls(conn, { hostname });
const w = new BufWriter(conn);
const r = new BufReader(conn);
const body = `GET / HTTP/1.1\r\nHost: ${hostname}:${port}\r\n\r\n`;
const writeResult = await w.write(encoder.encode(body));
assertEquals(body.length, writeResult);
await w.flush();
const tpr = new TextProtoReader(r);
const statusLine = await tpr.readLine();
assert(statusLine !== null, `line must be read: ${String(statusLine)}`);
const m = statusLine.match(/^(.+?) (.+?) (.+?)$/);
assert(m !== null, "must be matched");
const [_, proto, status, ok] = m;
assertEquals(proto, "HTTP/1.1");
assertEquals(status, "200");
assertEquals(ok, "OK");
const headers = await tpr.readMimeHeader();
assert(headers !== null);
const contentLength = parseInt(headers.get("content-length"));
const bodyBuf = new Uint8Array(contentLength);
await r.readFull(bodyBuf);
assertEquals(decoder.decode(bodyBuf), "Hello World\n");
conn.close();
listener.close();
await promise;

console.log("DONE");
