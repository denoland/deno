// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert } from "./test_util.ts";

test(function resourcesStdio(): void {
  const res = Deno.resources();

  assert.equals(res[0], "stdin");
  assert.equals(res[1], "stdout");
  assert.equals(res[2], "stderr");
});

testPerm({ net: true }, async function resourcesNet(): Promise<void> {
  const listener = Deno.listen({ port: 4501 });
  const dialerConn = await Deno.connect({ port: 4501 });
  const listenerConn = await listener.accept();

  const res = Deno.resources();
  assert.equals(
    Object.values(res).filter((r): boolean => r === "tcpListener").length,
    1
  );
  assert.equals(
    Object.values(res).filter((r): boolean => r === "tcpStream").length,
    2
  );

  listenerConn.close();
  dialerConn.close();
  listener.close();
});

testPerm({ read: true }, async function resourcesFile(): Promise<void> {
  const resourcesBefore = Deno.resources();
  await Deno.open("cli/tests/hello.txt");
  const resourcesAfter = Deno.resources();

  // check that exactly one new resource (file) was added
  assert.equals(
    Object.keys(resourcesAfter).length,
    Object.keys(resourcesBefore).length + 1
  );
  const newRid = +Object.keys(resourcesAfter).find((rid): boolean => {
    return !resourcesBefore.hasOwnProperty(rid);
  })!;
  assert.equals(resourcesAfter[newRid], "fsFile");
});
