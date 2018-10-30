// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

test(function stdioResources() {
  const res = deno.resources();

  assertEqual(res[0].rid, 0);
  assertEqual(res[0].repr, "stdin");

  assertEqual(res[1].rid, 1);
  assertEqual(res[1].repr, "stdout");

  assertEqual(res[2].rid, 2);
  assertEqual(res[2].repr, "stderr");
});

testPerm({ net: true }, async function netResources() {
  const addr = "127.0.0.1:4500";
  const listener = deno.listen("tcp", addr);

  listener.accept().then(async conn => {
    const res = deno.resources();
    // besides 3 stdio resources, we should have additional 3 from listen(), accept() and dial()
    assertEqual(res.length, 6);
    assertEqual(res.filter(r => r.repr === "tcpListener").length, 1);
    assertEqual(res.filter(r => r.repr === "tcpStream").length, 2);

    conn.close();
  });

  const conn = await deno.dial("tcp", addr);
  const buf = new Uint8Array(1024);
  await conn.read(buf);
});

test(function fileResources() {
  deno.readFileSync("package.json");
  const res = deno.resources();
  assertEqual(res.filter(r => r.repr === "fsFile").length, 1);
});
