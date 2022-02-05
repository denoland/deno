// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "./test_util.ts";

Deno.test(function resourcesCloseBadArgs() {
  assertThrows(() => {
    Deno.close((null as unknown) as number);
  }, TypeError);
});

Deno.test(function resourcesStdio() {
  const res = Deno.resources();

  assertEquals(res[0], "stdin");
  assertEquals(res[1], "stdout");
  assertEquals(res[2], "stderr");
});

Deno.test({ permissions: { net: true } }, async function resourcesNet() {
  const listener = Deno.listen({ port: 4501 });
  const dialerConn = await Deno.connect({ port: 4501 });
  const listenerConn = await listener.accept();

  const res = Deno.resources();
  assertEquals(
    Object.values(res).filter((r): boolean => r === "tcpListener").length,
    1,
  );
  const tcpStreams = Object.values(res).filter(
    (r): boolean => r === "tcpStream",
  );
  assert(tcpStreams.length >= 2);

  listenerConn.close();
  dialerConn.close();
  listener.close();
});

Deno.test({ permissions: { read: true } }, async function resourcesFile() {
  const resourcesBefore = Deno.resources();
  const f = await Deno.open("cli/tests/testdata/hello.txt");
  const resourcesAfter = Deno.resources();
  f.close();

  // check that exactly one new resource (file) was added
  assertEquals(
    Object.keys(resourcesAfter).length,
    Object.keys(resourcesBefore).length + 1,
  );
  const newRid = +Object.keys(resourcesAfter).find((rid): boolean => {
    return !Object.prototype.hasOwnProperty.call(resourcesBefore, rid);
  })!;
  assertEquals(resourcesAfter[newRid], "fsFile");
});
