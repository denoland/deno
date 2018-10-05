// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import * as deno from "deno";
import { testPerm, assert, assertEqual } from "./test_util.ts";

testPerm({ net: true }, function netListenClose() {
  const listener = deno.listen("tcp", "127.0.0.1:4500");
  listener.close();
});

testPerm({ net: true }, async function netDialListen() {
  const addr = "127.0.0.1:4500";
  const listener = deno.listen("tcp", addr);
  listener.accept().then(async conn => {
    await conn.write(new Uint8Array([1, 2, 3]));
    conn.close();
  });
  const conn = await deno.dial("tcp", addr);
  const buf = new Uint8Array(1024);
  const readResult = await conn.read(buf);
  assertEqual(3, readResult.nread);
  assertEqual(1, buf[0]);
  assertEqual(2, buf[1]);
  assertEqual(3, buf[2]);

  // TODO Currently ReadResult does not properly transmit EOF in the same call.
  // it requires a second call to get the EOF. Either ReadResult to be an
  // integer in which 0 signifies EOF or the handler should be modified so that
  // EOF is properly transmitted.
  assertEqual(false, readResult.eof);

  const readResult2 = await conn.read(buf);
  assertEqual(true, readResult2.eof);

  listener.close();
  conn.close();
});
