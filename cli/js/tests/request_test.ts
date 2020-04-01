// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(function fromInit(): void {
  const req = new Request("https://example.com", {
    body: "ahoyhoy",
    method: "POST",
    headers: {
      "test-header": "value",
    },
  });

  // @ts-ignore
  assertEquals("ahoyhoy", req._bodySource);
  assertEquals(req.url, "https://example.com");
  assertEquals(req.headers.get("test-header"), "value");
});

unitTest(function fromRequest(): void {
  const r = new Request("https://example.com");
  // @ts-ignore
  r._bodySource = "ahoyhoy";
  r.headers.set("test-header", "value");

  const req = new Request(r);

  // @ts-ignore
  assertEquals(req._bodySource, r._bodySource);
  assertEquals(req.url, r.url);
  assertEquals(req.headers.get("test-header"), r.headers.get("test-header"));
});

unitTest(async function cloneRequestBodyStream(): Promise<void> {
  // hack to get a stream
  const stream = new Request("", { body: "a test body" }).body;
  const r1 = new Request("https://example.com", {
    body: stream,
  });

  const r2 = r1.clone();

  const b1 = await r1.text();
  const b2 = await r2.text();

  assertEquals(b1, b2);

  // @ts-ignore
  assert(r1._bodySource !== r2._bodySource);
});
