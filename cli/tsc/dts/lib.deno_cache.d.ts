// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** The global {@linkcode CacheStorage} instance, providing access to the named
 * {@linkcode Cache} objects used to store and retrieve `Request`/`Response`
 * pairs.
 *
 * @category Cache */
declare var caches: typeof globalThis extends { document: any; caches: infer T }
  ? T
  : CacheStorage;

/** Represents the storage for named {@linkcode Cache} objects. It provides the
 * methods used to open, enumerate, look up, and delete caches, and is accessed
 * via the global {@linkcode caches} property.
 *
 * @category Cache */
interface CacheStorage {
  /** Open a cache storage for the provided name. */
  open(cacheName: string): Promise<Cache>;
  /** Check if cache already exists for the provided name. */
  has(cacheName: string): Promise<boolean>;
  /** Delete cache storage for the provided name. */
  delete(cacheName: string): Promise<boolean>;
  /** Return an array of all cache names tracked by the cache storage. */
  keys(): Promise<string[]>;
  /**
   * Check if a given `Request` or URL string is a key for a stored `Response`.
   * Returns the matching `Response`, or `undefined` if no match is found.
   *
   * If `options.cacheName` is provided, only the cache with that name is
   * searched. Otherwise, all caches are searched in creation order.
   */
  match(
    request: RequestInfo | URL,
    options?: MultiCacheQueryOptions,
  ): Promise<Response | undefined>;
}

/** Represents a single named store of `Request`/`Response` pairs. Obtain a
 * `Cache` via {@linkcode CacheStorage.open} and use it to persist responses and
 * later match incoming requests against them.
 *
 * @category Cache */
interface Cache {
  /**
   * Put the provided request/response into the cache.
   *
   * How is the API different from browsers?
   * 1. You cannot match cache objects using relative paths.
   * 2. You cannot pass options like `ignoreVary`, `ignoreMethod`, `ignoreSearch`.
   */
  put(request: RequestInfo | URL, response: Response): Promise<void>;
  /**
   * Return cache object matching the provided request.
   *
   * How is the API different from browsers?
   * 1. You cannot match cache objects using relative paths.
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
   * 1. You cannot delete cache objects using relative paths.
   * 2. You cannot pass options like `ignoreVary`, `ignoreMethod`, `ignoreSearch`.
   */
  delete(
    request: RequestInfo | URL,
    options?: CacheQueryOptions,
  ): Promise<boolean>;
  /**
   * Return the {@linkcode Request} keys stored in the cache, in insertion
   * order. When a `request` is provided, only the matching keys are returned.
   *
   * How is the API different from browsers?
   * 1. You cannot match cache objects using relative paths.
   * 2. You cannot pass options like `ignoreVary`, `ignoreMethod`, `ignoreSearch`.
   */
  keys(
    request?: RequestInfo | URL,
    options?: CacheQueryOptions,
  ): Promise<ReadonlyArray<Request>>;
}

/** The constructor object for {@linkcode Cache}.
 *
 * `Cache` instances are obtained via {@linkcode CacheStorage.open} rather than
 * constructed directly, so calling the constructor throws.
 *
 * @category Cache */
declare var Cache: typeof globalThis extends { document: any; Cache: infer T }
  ? T
  : {
    readonly prototype: Cache;
    new (): never;
  };

/** The constructor object for {@linkcode CacheStorage}.
 *
 * The `CacheStorage` instance is accessed via the global {@linkcode caches}
 * property rather than constructed directly, so calling the constructor throws.
 *
 * @category Cache */
declare var CacheStorage: typeof globalThis extends
  { document: any; CacheStorage: infer T } ? T : {
  readonly prototype: CacheStorage;
  new (): never;
};

/** @category Cache */
interface CacheQueryOptions {
  ignoreMethod?: boolean;
  ignoreSearch?: boolean;
  ignoreVary?: boolean;
}

/** @category Cache */
interface MultiCacheQueryOptions extends CacheQueryOptions {
  cacheName?: string;
}
