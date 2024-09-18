# deno_cache

This crate implements the Cache API for Deno.

The following APIs are implemented:

- [`CacheStorage::open()`][cache_storage_open]
- [`CacheStorage::has()`][cache_storage_has]
- [`CacheStorage::delete()`][cache_storage_delete]
- [`Cache::match()`][cache_match]
- [`Cache::put()`][cache_put]
- [`Cache::delete()`][cache_delete]

Cache APIs don't support the [query options][query_options] yet.

Spec: https://w3c.github.io/ServiceWorker/#cache-interface

[query_options]: https://w3c.github.io/ServiceWorker/#dictdef-cachequeryoptions
[cache_storage_open]: https://developer.mozilla.org/en-US/docs/Web/API/CacheStorage/open
[cache_storage_has]: https://developer.mozilla.org/en-US/docs/Web/API/CacheStorage/has
[cache_storage_delete]: https://developer.mozilla.org/en-US/docs/Web/API/CacheStorage/delete
[cache_match]: https://developer.mozilla.org/en-US/docs/Web/API/Cache/match
[cache_put]: https://developer.mozilla.org/en-US/docs/Web/API/Cache/put
[cache_delete]: https://developer.mozilla.org/en-US/docs/Web/API/Cache/delete
