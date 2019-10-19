import { assert, assertEquals } from "./cli/js/test_util.ts";
import { BufReader, BufWriter } from "./std/io/bufio.ts";
import { TextProtoReader } from "./std/textproto/mod.ts";

const conn = await Deno.dialTLS({
  hostname: "localhost",
  port: 4500,
  certFile: "./RootCA.pem"
});
assert(conn.rid > 0);
const w = new BufWriter(conn);
const r = new BufReader(conn);
let body = "GET / HTTP/1.1\r\n";
body += "Host: localhost:4500\r\n";
body += "\r\n";
const writeResult = await w.write(new TextEncoder().encode(body));
assertEquals(body.length, writeResult);
await w.flush();
const tpr = new TextProtoReader(r);
const statusLine = await tpr.readLine();
assert(!!statusLine, "line must be read: " + statusLine);
const m = statusLine.match(/^(.+?) (.+?) (.+?)$/);
assert(m !== null, "must be matched");
const [_, proto, status, ok] = m;
assertEquals(proto, "HTTP/1.1");
assertEquals(status, "200");
assertEquals(ok, "OK");
const headers = await tpr.readMIMEHeader();
const contentLength = parseInt(headers.get("content-length"));
const bodyBuf = new Uint8Array(contentLength);
await r.readFull(bodyBuf);
console.log("read body", new TextDecoder().decode(bodyBuf));
conn.close();
