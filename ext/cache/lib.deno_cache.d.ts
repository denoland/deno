// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Cache API */
declare var caches: CacheStorage;

/** @category Cache API */
declare interface CacheStorage {
  open(cacheName: string): Promise<Cache>;
  has(cacheName: string): Promise<boolean>;
  delete(cacheName: string): Promise<boolean>;
}

/** @category Cache API */
declare interface Cache {
  put(request: RequestInfo | URL, response: Response): Promise<void>;
  match(request: RequestInfo | URL): Promise<Response | undefined>;
  delete(request: RequestInfo | URL): Promise<boolean>;
}

/** @category Cache API */
declare var Cache: {
  prototype: Cache;
  new (name: string): Cache;
};

/** @category Cache API */
declare var CacheStorage: {
  prototype: CacheStorage;
  new (): CacheStorage;
};
