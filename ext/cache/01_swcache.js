// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../fetch/lib.deno_fetch.d.ts" />

"use strict";

((window) => {
  const {
    ArrayPrototypeJoin,
    Map,
    MapPrototypeDelete,
    MapPrototypeGet,
    MapPrototypeSet,
    ObjectFromEntries,
    PromiseResolve,
  } = window.__bootstrap.primordials;
  const { Request, Response } = window.__bootstrap.fetch;

  class Caches {
    constructor(engine) {
      this.engine = engine;
    }

    get default() {
      return new Cache(this.engine, "default");
    }

    async open(cacheNs) {
      await this.engine.open(cacheNs);
      return new Cache(this.engine, cacheNs);
    }
  }

  class Cache {
    constructor(engine, cacheNs) {
      this.engine = engine;
      this.cacheNs = cacheNs;
    }

    match(reqOrUrl, _cacheOptions) {
      const key = toKey(reqOrUrl);
      return this.engine.get(this.cacheNs, key);
    }

    put(reqOrUrl, resp) {
      const key = toKey(reqOrUrl);
      return this.engine.set(this.cacheNs, key, resp);
    }

    delete(reqOrUrl) {
      const key = toKey(reqOrUrl);
      return this.engine.del(this.cacheNs, key);
    }

    /// Other methods are intentionally unsupported, for now
  }

  function toKey(reqOrUrl) {
    return reqToKey(asReq(reqOrUrl));
  }

  function asReq(reqOrUrl) {
    return reqOrUrl instanceof Request ? reqOrUrl : new Request(reqOrUrl);
  }

  function reqToKey(req) {
    return ArrayPrototypeJoin([req.url, req.method], ":");
  }

  // A no-op cache engine, cache-disabled
  class VoidEngine {
    open(_cacheNs) {
      return PromiseResolve();
    }
    set(_cacheNs, _key, _resp) {
      return PromiseResolve();
    }
    get(_cacheNs, _key) {
      return PromiseResolve();
    }
    del(_cacheNs, _key) {
      return PromiseResolve(false);
    }
  }

  // Incredibly naive cache-engine, in-memory and unbounded
  class NaiveEngine {
    constructor() {
      this.kvs = new Map();
    }

    open(_cacheNs) {
      return PromiseResolve();
    }

    async set(cacheNs, key, resp) {
      MapPrototypeSet(
        this.kvs,
        this.key(cacheNs, key),
        await this.persisted(resp),
      );
    }

    get(cacheNs, key) {
      const resp = MapPrototypeGet(this.kvs, this.key(cacheNs, key));
      return PromiseResolve(resp ? this.fromPersisted(resp) : undefined);
    }

    del(cacheNs, key) {
      return PromiseResolve(
        MapPrototypeDelete(this.kvs, this.key(cacheNs, key)),
      );
    }

    key(cacheNs, key) {
      return `${cacheNs}:${key}`;
    }

    fromPersisted(pResp) {
      const { body, ...init } = pResp;
      return new Response(body, init);
    }

    async persisted(resp) {
      return {
        body: await resp.arrayBuffer(),
        headers: ObjectFromEntries(resp.headers.entries()),
        status: resp.status,
        statusText: resp.statusText,
      };
    }
  }

  window.__bootstrap.cache = {
    Cache,
    Caches,
    NaiveEngine,
    VoidEngine,
  };
})(globalThis);
