# deno_cache

This crate implements the Cache API for Deno.

The following APIs are implemented:

- `CacheStorage::open()`
- `CacheStorage::has()`
- `CacheStorage::delete()`

Cache APIs don't support the [query options][queryoptions] yet.

- `Cache::match()`
- `Cache::put()`
- `Cache::delete()`

Spec: https://w3c.github.io/ServiceWorker/#cache-interface

[queryoptions]: https://w3c.github.io/ServiceWorker/#dictdef-cachequeryoptions
