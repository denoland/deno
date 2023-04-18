// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import http from "node:http";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import { assertSpyCalls, spy } from "../../../test_util/std/testing/mock.ts";
import { deferred } from "../../../test_util/std/async/deferred.ts";

Deno.test("[node/http] ServerResponse _implicitHeader", async () => {
  const d = deferred<void>();
  const server = http.createServer((_req, res) => {
    const writeHeadSpy = spy(res, "writeHead");
    // deno-lint-ignore no-explicit-any
    (res as any)._implicitHeader();
    assertSpyCalls(writeHeadSpy, 1);
    writeHeadSpy.restore();
    res.end("Hello World");
  });

  server.listen(async () => {
    const { port } = server.address() as { port: number };
    const res = await fetch(`http://localhost:${port}`);
    assertEquals(await res.text(), "Hello World");
    server.close(() => {
      d.resolve();
    });
  });

  await d;
});
