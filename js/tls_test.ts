// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEquals } from "./test_util.ts";

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
  const body = new TextEncoder().encode("GET / HTTP/1.0\r\n\r\n");
  const writeResult = await conn.write(body);
  assertEquals(body.length, writeResult);
  conn.close();
});
