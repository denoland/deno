// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import { assert, assertFalse } from "./test_util.ts";

Deno.test(
  { permissions: {} },
  async function cacheStorage() {
    const cacheName = "cache-v1";
    assertFalse(await caches.has(cacheName));
    const _cache = await caches.open(cacheName);
    assert(await caches.has(cacheName));
    assert(await caches.delete(cacheName));
  },
);

Deno.test(
  { permissions: {} },
  async function cacheApi() {
    const cacheName = "cache-v1";
    assertFalse(await caches.has(cacheName));
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

    await caches.delete(cacheName);
  },
);
