// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function fromInit(): void {
  const req = new Request("https://example.com", {
    body: "ahoyhoy",
    method: "POST",
    headers: {
      "test-header": "value"
    }
  });

  // @ts-ignore
  assert.equals("ahoyhoy", req._bodySource);
  assert.equals(req.url, "https://example.com");
  assert.equals(req.headers.get("test-header"), "value");
});

test(function fromRequest(): void {
  const r = new Request("https://example.com");
  // @ts-ignore
  r._bodySource = "ahoyhoy";
  r.headers.set("test-header", "value");

  const req = new Request(r);

  // @ts-ignore
  assert.equals(req._bodySource, r._bodySource);
  assert.equals(req.url, r.url);
  assert.equals(req.headers.get("test-header"), r.headers.get("test-header"));
});

test(async function cloneRequestBodyStream(): Promise<void> {
  // hack to get a stream
  const stream = new Request("", { body: "a test body" }).body;
  const r1 = new Request("https://example.com", {
    body: stream
  });

  const r2 = r1.clone();

  const b1 = await r1.text();
  const b2 = await r2.text();

  assert.equals(b1, b2);

  // @ts-ignore
  assert(r1._bodySource !== r2._bodySource);
});
