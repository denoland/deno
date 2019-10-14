// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEquals } from "./test_util.ts";
import { BufWriter, BufReader } from "../../std/io/bufio.ts";
import { TextProtoReader } from "../../std/textproto/mod.ts";
import { runIfMain } from "../../std/testing/mod.ts";
// TODO(ry) The tests in this file use github.com:443, but it would be better to
// not rely on an internet connection and rather use a localhost TLS server.

test(async function dialTLSNoPerm(): Promise<void> {
  let err;
  try {
    await Deno.dialTLS({ hostname: "github.com", port: 443 });
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ net: true }, async function dialTLSBasic(): Promise<void> {
  const conn = await Deno.dialTLS({ hostname: "github.com", port: 443 });
  assert(conn.rid > 0);
  const w = new BufWriter(conn);
  const r = new BufReader(conn);
  let body = "GET / HTTP/1.1\r\n";
  body += "Host: github.com\r\n";
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
  conn.close();
});

runIfMain(import.meta);
