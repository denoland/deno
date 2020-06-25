// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals, assert, assertThrows } from "./test_util.ts";

unitTest(function resourcesCloseBadArgs(): void {
  assertThrows(() => {
    Deno.close((null as unknown) as number);
  }, Deno.errors.InvalidData);
});

unitTest(function resourcesStdio(): void {
  const res = Deno.resources();

  assertEquals(res[0], "stdin");
  assertEquals(res[1], "stdout");
  assertEquals(res[2], "stderr");
});

unitTest({ perms: { net: true } }, async function resourcesNet(): Promise<
  void
> {
  const listener = Deno.listen({ port: 4501 });
  const dialerConn = await Deno.connect({ port: 4501 });
  const listenerConn = await listener.accept();

  const res = Deno.resources();
  assertEquals(
    Object.values(res).filter((r): boolean => r === "tcpListener").length,
    1
  );
  const tcpStreams = Object.values(res).filter(
    (r): boolean => r === "tcpStream"
  );
  assert(tcpStreams.length >= 2);

  listenerConn.close();
  dialerConn.close();
  listener.close();
});

unitTest({ perms: { read: true } }, async function resourcesFile(): Promise<
  void
> {
  const resourcesBefore = Deno.resources();
  const f = await Deno.open("cli/tests/hello.txt");
  const resourcesAfter = Deno.resources();
  f.close();

  // check that exactly one new resource (file) was added
  assertEquals(
    Object.keys(resourcesAfter).length,
    Object.keys(resourcesBefore).length + 1
  );
  const newRid = +Object.keys(resourcesAfter).find((rid): boolean => {
    return !resourcesBefore.hasOwnProperty(rid);
  })!;
  assertEquals(resourcesAfter[newRid], "fsFile");
});
