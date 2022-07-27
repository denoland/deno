// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

((window) => {
  function queryCache(requestQuery, options = {}, targetStorage = new Map()) {
    const resultList = new Map();
    // let storage = null;
    // Ignore step 2-4;
    for (const [request, response] of targetStorage.entries()) {
      if (requestMatchesCatchedItem(requestQuery, request, response, options)) {
        resultList.set(request, response);
      }
    }
    return resultList;
  }

  function requestMatchesCachedItem(
    requestQuery,
    request,
    response = null,
    options = {},
  ) {
    // Step 1.
    if (options["ignoreMethod"] === false && request.method !== "GET") {
      return false;
    }

    // Step 2.
    let queryURL = requestQuery.url;
    let cachedURL = request.url;
    if (options["ignoreSearch"] === true) {
      queryURL = "";
      cachedURL = "";
    }

    // Step 5.
    {
      const a = new URL(queryURL);
      const b = new URL(cachedURL);
      if (
        a.host !== b.host || a.pathname !== b.pathname || a.search !== b.search
      ) {
        // TODO(@satyarohith): think about exclude fragment flag
        return false;
      }
    }

    // Step 6.
    if (
      (response === null && options["ignoreVary"] === true) ||
      !response.headers.has("Vary")
    ) {
      return true;
    }

    // Step 7.
    const varyHeader = response.headers.get("Vary");
    // TODO(@satyarohith): do the parsing of the vary header.
    const fieldValues = varyHeader.split(",").map((field) => field.trim());
    for (const fieldValue of fieldValues) {
      if (
        fieldValue === "*" ||
        request.headers.get(fieldValue) !== requestQuery.headers.get(fieldValue)
      ) {
        return false;
      }
    }

    return true;
  }

  class CacheStorage {
    #storage;

    constructor() {
      this.#storage = new Map();
      return this;
    }

    // deno-lint-ignore require-await
    async match(_request, _options) {
      // TODO(@satyarohith): implement the algorithm.
      return Promise.resolve(new Response("hello world"));
    }
    // deno-lint-ignore require-await
    async open(cacheName) {
      if (!this.#storage.has(cacheName)) {
        this.#storage.set(cacheName, new Cache(cacheName));
      }
      return Promise.resolve(this.#storage.get(cacheName));
    }
    // deno-lint-ignore require-await
    async has(cacheName) {
      return Promise.resolve(this.#storage.has(cacheName));
    }
    // deno-lint-ignore require-await
    async delete(cacheName) {
      return Promise.resolve(this.#storage.delete(cacheName));
    }
    // deno-lint-ignore require-await
    async keys() {
      return Promise.resolve(Array.from(this.#storage.keys()));
    }
  }

  class Cache {
    #storage;
    #name;
    constructor(cacheName) {
      this.#name = cacheName;
      this.#storage = new Map();
      return this;
    }
    // async match(request, options) {}

    // deno-lint-ignore require-await
    async matchAll(request, options = {}) {
      let r = null;
      // Step 2.
      if (request instanceof Request) {
        if (request.method !== "GET" && !options?.ignoreMethod) {
          return Promise.resolve([]);
        }
        r = request;
      } else if (request instanceof string) {
        try {
          r = new Request(request);
        } catch (error) {
          return Promise.reject(error);
        }
      }

      // Step 5.
      const responses = [];
      // Step 5.2
      if (r === null) {
        for (const [_request, response] of this.#storage.entries()) {
          responses.push(response);
        }
        // Step 5.3
      } else {
        const requestResponses = queryCache(r, options, this.#storage);
        for (const response of requestResponses.values()) {
          responses.push(response);
        }
        // Skip 5.4.
      }
      // Step 5.5

      return Promise.resolve(responses);
    }

    // deno-lint-ignore require-await
    async add(request) {
      const requests = [request];
      return this.addAll(requests);
    }

    // async addAll(requests) {
    //   const responsePromises = [];
    //   const requestList = [];
    //   for (const request of requests) {
    //     if (
    //       request instanceof Request &&
    //         request.scheme !== "http" && request.scheme !== "https" ||
    //       request.method !== "GET"
    //     ) {
    //       return Promise.reject(new TypeError("type error"));
    //     }
    //   }
    // }

    // put(request, response) {
    //   let innerRequest = null;
    //   if (request instanceof Request) {
    //     innerRequest = request;
    //   } else {
    //     try {
    //       innerRequest = new Request(request);
    //     } catch (error) {
    //       throw Promise.reject(error);
    //     }
    //   }
    // }

    // async delete(request, options) {}
    // deno-lint-ignore require-await
    async keys() {
      return Promise.resolve(Array.from(this.#storage.keys()));
    }
  }

  window.__bootstrap.caches = {
    CacheStorage,
  };
})(this);
