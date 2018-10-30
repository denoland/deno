// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

test(function stdioResources() {
  const res = deno.resources();

  assertEqual(Object.keys(res).length, 3);
  assertEqual(res[0], "stdin");
  assertEqual(res[1], "stdout");
  assertEqual(res[2], "stderr");
});

testPerm({ net: true }, async function netResources() {
  const addr = "127.0.0.1:4500";
  const listener = deno.listen("tcp", addr);

  listener.accept().then(async conn => {
    const res = deno.resources();
    // besides 3 stdio resources, we should have additional 3 from listen(), accept() and dial()
    assertEqual(Object.keys(res).length, 6);
    assertEqual(Object.values(res).filter(r => r === "tcpListener").length, 1);
    assertEqual(Object.values(res).filter(r => r === "tcpStream").length, 2);

    conn.close();
    listener.close();
  });

  const conn = await deno.dial("tcp", addr);
  const buf = new Uint8Array(1024);
  await conn.read(buf);
  conn.close();
});

test(function fileResources() {
  deno.open("package.json");
  const res = deno.resources();
  console.log(res);
  assertEqual(Object.values(res).filter(r => r === "fsFile").length, 1);
});
