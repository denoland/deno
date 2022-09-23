// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

const cacheName = "cache-v1";
const cache = await caches.open(cacheName);
const req = "https://deno.com";

Deno.bench(
  `cache_storage_open`,
  { n: 5e2 },
  async () => {
    await caches.open("cache-v2");
  },
);

Deno.bench(
  `cache_storage_has`,
  { n: 5e2 },
  async () => {
    await caches.has("cache-v2");
  },
);

Deno.bench(
  `cache_storage_delete`,
  { n: 5e2 },
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
  { n: 5e2 },
  async () => {
    await cache.put(req, new Response(body));
  },
);

Deno.bench("cache_put_no_body", { n: 5e2 }, async () => {
  await cache.put(
    "https://deno.land/redirect",
    Response.redirect("https://deno.com"),
  );
});

Deno.bench("cache_match", { n: 5e2 }, async () => {
  await cache.match(req);
});

Deno.bench("cache_delete", { n: 5e2 }, async () => {
  await cache.delete(req);
});
