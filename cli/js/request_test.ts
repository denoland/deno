// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assertEquals } from "./test_util.ts";

test(function fromInit(): void {
  const req = new Request("https://example.com", {
    body: "ahoyhoy",
    method: "POST",
    headers: {
      "test-header": "value"
    }
  });

  // @ts-ignore
  assertEquals("ahoyhoy", req._bodySource);
  assertEquals(req.url, "https://example.com");
  assertEquals(req.headers.get("test-header"), "value");
});
