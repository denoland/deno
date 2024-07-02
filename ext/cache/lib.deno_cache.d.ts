// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** Returns the CacheStorage object
 * associated with the current context.
 *
 * @category Cache
 */
declare var caches: CacheStorage;

/** Represents the storage for Cache objects.
 * The interface maintains a mapping of string names to corresponding
 * Cache objects.
 *
 * Use CacheStorage.open() to obtain a Cache instance.
 *
 * @category Cache
 */
declare interface CacheStorage {
  /** Open a cache storage for the provided name. */
  open(cacheName: string): Promise<Cache>;
  /** Check if cache already exists for the provided name. */
  has(cacheName: string): Promise<boolean>;
  /** Delete cache storage for the provided name. */
  delete(cacheName: string): Promise<boolean>;
}

/**
 * Provides a storage mechanism for Request / Response object pairs
 *
 * @category Cache
 */
declare interface Cache {
  /**
   * Put the provided request/response into the cache.
   *
   * How is the API different from browsers?
   * 1. You cannot match cache objects using by relative paths.
   * 2. You cannot pass options like `ignoreVary`, `ignoreMethod`, `ignoreSearch`.
   */
  put(request: RequestInfo | URL, response: Response): Promise<void>;
  /**
   * Return cache object matching the provided request.
   *
   * How is the API different from browsers?
   * 1. You cannot match cache objects using by relative paths.
   * 2. You cannot pass options like `ignoreVary`, `ignoreMethod`, `ignoreSearch`.
   */
  match(
    request: RequestInfo | URL,
    options?: CacheQueryOptions,
  ): Promise<Response | undefined>;
  /**
   * Delete cache object matching the provided request.
   *
   * How is the API different from browsers?
   * 1. You cannot delete cache objects using by relative paths.
   * 2. You cannot pass options like `ignoreVary`, `ignoreMethod`, `ignoreSearch`.
   */
  delete(
    request: RequestInfo | URL,
    options?: CacheQueryOptions,
  ): Promise<boolean>;
}

/** @category Cache */
declare var Cache: {
  readonly prototype: Cache;
  new (): never;
};

/** @category Cache */
declare var CacheStorage: {
  readonly prototype: CacheStorage;
  new (): never;
};

/** @category Cache */
declare interface CacheQueryOptions {
  ignoreMethod?: boolean;
  ignoreSearch?: boolean;
  ignoreVary?: boolean;
}
