// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

import {
  Buffer,
  BufReader,
  BufWriter,
} from "../../../test_util/std/io/buffer.ts";
import { TextProtoReader } from "../../../test_util/std/textproto/mod.ts";
import { serve, serveTls } from "../../../test_util/std/http/server.ts";
import {
  assert,
  assertEquals,
  assertRejects,
  assertStrictEquals,
  assertThrows,
  deferred,
  delay,
  fail,
} from "./test_util.ts";

Deno.test({ permissions: { net: true } }, async function httpServerBasic() {
  const promise = (async () => {
    await Deno.serve(async (request) => {
      console.log("request url", request.url, request.method);
      assertEquals(new URL(request.url).href, "http://127.0.0.1:4501/");
      assertEquals(await request.text(), "");
      return new Response("Hello World", { headers: { "foo": "bar" } });
    }, { port: 4501 });
  })();

  const resp = await fetch("http://127.0.0.1:4501/", {
    headers: { "connection": "close" },
  });
  console.log("after fetch");
  const clone = resp.clone();
  const text = await resp.text();
  console.log("text", text);
  console.log("text", resp.headers);
  assertEquals(text, "Hello World");
  assertEquals(resp.headers.get("foo"), "bar");
  const cloneText = await clone.text();
  assertEquals(cloneText, "Hello World");
  console.log("before close");
  await promise;
  console.log("finished");
});
