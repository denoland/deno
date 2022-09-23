// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertFalse,
  assertRejects,
} from "./test_util.ts";

Deno.test(async function cacheStorage() {
  const cacheName = "cache-v1";
  const _cache = await caches.open(cacheName);
  assert(await caches.has(cacheName));
  assert(await caches.delete(cacheName));
  assertFalse(await caches.has(cacheName));
});

Deno.test(async function cacheApi() {
  const cacheName = "cache-v1";
  const cache = await caches.open(cacheName);
  // Test cache.put() with url string as key.
  {
    const req = "https://deno.com";
    await cache.put(req, new Response("deno.com - key is string"));
    const res = await cache.match(req);
    assertEquals(await res?.text(), "deno.com - key is string");
    assert(await cache.delete(req));
  }
  // Test cache.put() with url instance as key.
  {
    const req = new URL("https://deno.com");
    await cache.put(req, new Response("deno.com - key is URL"));
    const res = await cache.match(req);
    assertEquals(await res?.text(), "deno.com - key is URL");
    assert(await cache.delete(req));
  }
  // Test cache.put() with request instance as key.
  {
    const req = new Request("https://deno.com");
    await cache.put(req, new Response("deno.com - key is Request"));
    const res = await cache.match(req);
    assertEquals(await res?.text(), "deno.com - key is Request");
    assert(await cache.delete(req));
  }

  // Test cache.put() throws with response Vary header set to *.
  {
    const req = new Request("https://deno.com");
    assertRejects(
      async () => {
        await cache.put(
          req,
          new Response("deno.com - key is Request", {
            headers: { Vary: "*" },
          }),
        );
      },
      TypeError,
      "Vary header must not contain '*'",
    );
  }

  // Test cache.match() with same url but different values for Vary header.
  {
    await cache.put(
      new Request("https://example.com/", {
        headers: {
          "Accept": "application/json",
        },
      }),
      Response.json({ msg: "hello world" }, {
        headers: {
          "Content-Type": "application/json",
          "Vary": "Accept",
        },
      }),
    );
    const res = await cache.match("https://example.com/");
    assertEquals(res, undefined);
    const res2 = await cache.match(
      new Request("https://example.com/", {
        headers: { "Accept": "text/html" },
      }),
    );
    assertEquals(res2, undefined);

    const res3 = await cache.match(
      new Request("https://example.com/", {
        headers: { "Accept": "application/json" },
      }),
    );
    assertEquals(await res3?.json(), { msg: "hello world" });
  }

  assert(await caches.delete(cacheName));
  assertFalse(await caches.has(cacheName));
});
