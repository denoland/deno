// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assertEqual } from "./test_util.ts";

test(function resourcesStdio() {
  const res = Deno.resources();

  assertEqual(res[0], "stdin");
  assertEqual(res[1], "stdout");
  assertEqual(res[2], "stderr");
});

testPerm({ net: true }, async function resourcesNet() {
  const addr = "127.0.0.1:4501";
  const listener = Deno.listen("tcp", addr);

  const dialerConn = await Deno.dial("tcp", addr);
  const listenerConn = await listener.accept();

  const res = Deno.resources();
  assertEqual(Object.values(res).filter(r => r === "tcpListener").length, 1);
  assertEqual(Object.values(res).filter(r => r === "tcpStream").length, 2);

  listenerConn.close();
  dialerConn.close();
  listener.close();
});

testPerm({ read: true }, async function resourcesFile() {
  const resourcesBefore = Deno.resources();
  await Deno.open("tests/hello.txt");
  const resourcesAfter = Deno.resources();

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
