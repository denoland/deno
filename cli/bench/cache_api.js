// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

const cacheName = "cache-v1";
const cache = await caches.open(cacheName);
const req = "https://deno.com";

Deno.bench(
  `cache_storage_open`,
  async () => {
    await caches.open("cache-v2");
  },
);

Deno.bench(
  `cache_storage_has`,
  async () => {
    await caches.has("cache-v2");
  },
);

Deno.bench(
  `cache_storage_delete`,
  async () => {
    await caches.delete("cache-v2");
  },
);

// 100 bytes.
const loremIpsum =
  `Lorem ipsum dolor sit amet, consectetur adipiscing…es ligula in libero. Sed dignissim lacinia nunc. `;
let body;
for (let index = 1; index <= 110; index++) {
  body += loremIpsum;
}

Deno.bench(
  `cache_put_body_${Math.floor(body.length / 1024)}_KiB`,
  async () => {
    await cache.put(req, new Response(body));
  },
);

Deno.bench("cache_put_no_body", async () => {
  await cache.put(
    "https://deno.land/redirect",
    Response.redirect("https://deno.com"),
  );
});

Deno.bench("cache_match", async () => {
  await cache.match(req);
});

Deno.bench("cache_delete", async () => {
  await cache.delete(req);
});
