// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertFalse,
  assertRejects,
  assertThrows,
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

Deno.test(function cacheIllegalConstructor() {
  assertThrows(() => new Cache(), TypeError, "Illegal constructor");
  // @ts-expect-error illegal constructor
  assertThrows(() => new Cache("foo", "bar"), TypeError, "Illegal constructor");
});

Deno.test(async function cachePutReaderLock() {
  const cacheName = "cache-v1";
  const cache = await caches.open(cacheName);

  const response = new Response("consumed");

  const promise = cache.put(
    new Request("https://example.com/"),
    response,
  );

  await assertRejects(
    async () => {
      await response.arrayBuffer();
    },
    TypeError,
    "Body already consumed",
  );

  await promise;
});

Deno.test(async function cachePutResourceLeak() {
  const cacheName = "cache-v1";
  const cache = await caches.open(cacheName);

  const stream = new ReadableStream({
    start(controller) {
      controller.error(new Error("leak"));
    },
  });

  await assertRejects(
    async () => {
      await cache.put(
        new Request("https://example.com/leak"),
        new Response(stream),
      );
    },
    Error,
    "leak",
  );
});

Deno.test(async function cachePutFailedBody() {
  const cacheName = "cache-v1";
  const cache = await caches.open(cacheName);

  const request = new Request("https://example.com/failed-body");
  const stream = new ReadableStream({
    start(controller) {
      controller.error(new Error("corrupt"));
    },
  });

  await assertRejects(
    async () => {
      await cache.put(
        request,
        new Response(stream),
      );
    },
    Error,
    "corrupt",
  );

  const response = await cache.match(request);
  // if it fails to read the body, the cache should be empty
  assertEquals(response, undefined);
});

Deno.test(async function cachePutOverwrite() {
  const cacheName = "cache-v1";
  const cache = await caches.open(cacheName);

  const request = new Request("https://example.com/overwrite");
  const res1 = new Response("res1");
  const res2 = new Response("res2");

  await cache.put(request, res1);
  const res = await cache.match(request);
  assertEquals(await res?.text(), "res1");

  await cache.put(request, res2);
  const res_ = await cache.match(request);
  assertEquals(await res_?.text(), "res2");
});

// Ensure that we can successfully put a response backed by a resource
Deno.test(async function cachePutResource() {
  const tempFile = Deno.makeTempFileSync({ prefix: "deno-", suffix: ".txt" });
  Deno.writeTextFileSync(tempFile, "Contents".repeat(1024));

  const file = Deno.openSync(tempFile);

  const cacheName = "cache-v1";
  const cache = await caches.open(cacheName);

  const request = new Request("https://example.com/file");
  await cache.put(request, new Response(file.readable));
  const res = await cache.match(request);
  assertEquals(await res?.text(), "Contents".repeat(1024));
});
