// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ net: true }, async function resources() {
  const res = deno.resources();
  console.log('resources', res)
  // there should be only stdio
  assertEqual(res.length, 3);

  const stdio = [{rid: 0, repr: "stdin"}, {rid: 1, repr: "stdout"}, {rid: 2, repr: "stderr"}];

  stdio.forEach((stdioItem) => {
    const found = res.find(resItem => resItem.rid === stdioItem.rid && resItem.repr === stdioItem.repr);
    assert(!!found);
  });

  const addr = "127.0.0.1:4500";
  const listener = deno.listen("tcp", addr);
  listener.accept().then(async conn => {
    // TODO: this is failing, we have 6 resources open intead of 4
    const res1 = deno.resources();
    console.log('resources', res1)
    // there should be only stdio
    assertEqual(res1.length, 4);
    conn.close();
  });

  const conn = await deno.dial("tcp", addr);
  const buf = new Uint8Array(1024);
  await conn.read(buf);
});
