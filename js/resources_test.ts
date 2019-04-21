// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assertEquals } from "./test_util.ts";

test(function resourcesStdio(): void {
  const res = Deno.resources();

  assertEquals(res[0], "stdin");
  assertEquals(res[1], "stdout");
  assertEquals(res[2], "stderr");
});

testPerm({ net: true }, async function resourcesNet(): Promise<void> {
  const addr = "127.0.0.1:4501";
  const listener = Deno.listen("tcp", addr);

  const dialerConn = await Deno.dial("tcp", addr);
  const listenerConn = await listener.accept();

  const res = Deno.resources();
  assertEquals(
    Object.values(res).filter((r): boolean => r === "tcpListener").length,
    1
  );
  assertEquals(
    Object.values(res).filter((r): boolean => r === "tcpStream").length,
    2
  );

  listenerConn.close();
  dialerConn.close();
  listener.close();
});

testPerm({ read: true }, async function resourcesFile(): Promise<void> {
  const resourcesBefore = Deno.resources();
  await Deno.open("tests/hello.txt");
  const resourcesAfter = Deno.resources();

  // check that exactly one new resource (file) was added
  assertEquals(
    Object.keys(resourcesAfter).length,
    Object.keys(resourcesBefore).length + 1
  );
  const newRid = Object.keys(resourcesAfter).find(
    (rid): boolean => {
      return !resourcesBefore.hasOwnProperty(rid);
    }
  );
  assertEquals(resourcesAfter[newRid], "fsFile");
});
