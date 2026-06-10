// Copyright 2018-2026 the Deno authors. MIT license.
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

Deno.test(async function cacheStorageKeys() {
  const names = ["keys-a", "keys-b", "keys-c"];
  for (const name of names) {
    await caches.delete(name);
  }

  const before = await caches.keys();
  assert(Array.isArray(before));
  for (const name of names) {
    assertFalse(before.includes(name));
  }

  for (const name of names) {
    await caches.open(name);
  }

  const after = await caches.keys();
  for (const name of names) {
    assert(after.includes(name), `expected keys() to contain ${name}`);
  }

  assert(await caches.delete("keys-b"));
  const afterDelete = await caches.keys();
  assert(afterDelete.includes("keys-a"));
  assertFalse(afterDelete.includes("keys-b"));
  assert(afterDelete.includes("keys-c"));

  assert(await caches.delete("keys-a"));
  assert(await caches.delete("keys-c"));
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

Deno.test(async function cacheStorageMatch() {
  const names = ["match-a", "match-b", "match-c"];
  for (const name of names) {
    await caches.delete(name);
  }

  const cacheA = await caches.open("match-a");
  const cacheB = await caches.open("match-b");
  const cacheC = await caches.open("match-c");

  await cacheA.put(
    "https://example.com/only-a",
    new Response("from a"),
  );
  await cacheB.put(
    "https://example.com/shared",
    new Response("from b"),
  );
  await cacheC.put(
    "https://example.com/shared",
    new Response("from c"),
  );
  await cacheC.put(
    "https://example.com/only-c",
    new Response("from c only"),
  );

  // Matches across all caches.
  const resA = await caches.match("https://example.com/only-a");
  assertEquals(await resA?.text(), "from a");

  const resC = await caches.match("https://example.com/only-c");
  assertEquals(await resC?.text(), "from c only");

  // When multiple caches match, the earlier cache (in creation order) wins.
  const resShared = await caches.match("https://example.com/shared");
  assertEquals(await resShared?.text(), "from b");

  // Returns undefined when nothing matches.
  const resMissing = await caches.match("https://example.com/missing");
  assertEquals(resMissing, undefined);

  // Accepts URL and Request as key.
  const resUrl = await caches.match(new URL("https://example.com/only-a"));
  assertEquals(await resUrl?.text(), "from a");

  const resReq = await caches.match(
    new Request("https://example.com/only-a"),
  );
  assertEquals(await resReq?.text(), "from a");

  // cacheName restricts the search to that cache.
  const resScoped = await caches.match("https://example.com/shared", {
    cacheName: "match-c",
  });
  assertEquals(await resScoped?.text(), "from c");

  // cacheName that doesn't exist yields undefined, even if another cache
  // would have matched.
  const resUnknownCache = await caches.match("https://example.com/shared", {
    cacheName: "match-nonexistent",
  });
  assertEquals(resUnknownCache, undefined);

  for (const name of names) {
    assert(await caches.delete(name));
  }
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
