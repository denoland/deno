// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

test(function resourcesStdio() {
  const res = deno.resources();

  assertEqual(res[0], "stdin");
  assertEqual(res[1], "stdout");
  assertEqual(res[2], "stderr");
});

testPerm({ net: true }, async function resourcesNet() {
  const addr = "127.0.0.1:4501";
  const listener = deno.listen("tcp", addr);
  let counter = 0;

  listener.accept().then(async conn => {
    const res = deno.resources();
    // besides 3 stdio resources, we should have additional 3 from listen(), accept() and dial()
    assertEqual(Object.keys(res).length, 6);
    assertEqual(Object.values(res).filter(r => r === "tcpListener").length, 1);
    assertEqual(Object.values(res).filter(r => r === "tcpStream").length, 2);

    conn.close();
    listener.close();
    counter++;
  });

  const conn = await deno.dial("tcp", addr);
  conn.close();
  assertEqual(counter, 1);
});

test(async function resourcesFile() {
  const resourcesBefore = deno.resources();
  await deno.open("tests/hello.txt");
  const resourcesAfter = deno.resources();

  // check that exactly one new resource (file) was added
  assertEqual(
    Object.keys(resourcesAfter).length,
    Object.keys(resourcesBefore).length + 1
  );
  const newRid = Object.keys(resourcesAfter).find(rid => {
    return !resourcesBefore.hasOwnProperty(rid);
  });
  assertEqual(resourcesAfter[newRid], "fsFile");
});
