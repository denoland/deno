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

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  assertEquals("ahoyhoy", (req as any)._bodySource);
  assertEquals(req.url, "https://example.com");
  assertEquals(req.headers.get("test-header"), "value");
});

unitTest(function fromRequest(): void {
  const r = new Request("https://example.com");
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (r as any)._bodySource = "ahoyhoy";
  r.headers.set("test-header", "value");

  const req = new Request(r);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  assertEquals((req as any)._bodySource, (r as any)._bodySource);
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

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  assert((r1 as any)._bodySource !== (r2 as any)._bodySource);
});
