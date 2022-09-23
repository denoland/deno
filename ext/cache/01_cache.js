// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.__bootstrap.core;
  const webidl = window.__bootstrap.webidl;
  const {
    Symbol,
    TypeError,
    Uint8Array,
    ObjectPrototypeIsPrototypeOf,
  } = window.__bootstrap.primordials;
  const {
    Request,
    toInnerResponse,
    toInnerRequest,
  } = window.__bootstrap.fetch;
  const { URLPrototype } = window.__bootstrap.url;
  const RequestPrototype = Request.prototype;
  const { getHeader } = window.__bootstrap.headers;

  class CacheStorage {
    constructor() {
      webidl.illegalConstructor();
    }

    async open(cacheName) {
      webidl.assertBranded(this, CacheStoragePrototype);
      const prefix = "Failed to execute 'open' on 'CacheStorage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      const cacheId = await core.opAsync("op_cache_storage_open", cacheName);
      return new Cache(cacheId);
    }

    async has(cacheName) {
      webidl.assertBranded(this, CacheStoragePrototype);
      const prefix = "Failed to execute 'has' on 'CacheStorage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      return await core.opAsync("op_cache_storage_has", cacheName);
    }

    async delete(cacheName) {
      webidl.assertBranded(this, CacheStoragePrototype);
      const prefix = "Failed to execute 'delete' on 'CacheStorage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      return await core.opAsync("op_cache_storage_delete", cacheName);
    }
  }

  const _id = Symbol("id");

  class Cache {
    /** @type {number} */
    [_id];

    constructor(cacheId) {
      this[_id] = cacheId;
    }

    /** See https://w3c.github.io/ServiceWorker/#dom-cache-put */
    async put(request, response) {
      const prefix = "Failed to execute 'put' on 'Cache'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      // Step 1.
      let innerRequest = null;
      // Step 2.
      if (ObjectPrototypeIsPrototypeOf(RequestPrototype, request)) {
        innerRequest = toInnerRequest(request);
      } else {
        // Step 3.
        innerRequest = toInnerRequest(new Request(request));
      }
      // Step 4.
      const reqUrl = new URL(innerRequest.url());
      if (reqUrl.protocol !== "http:" && reqUrl.protocol !== "https:") {
        throw new TypeError(
          "Request url protocol must be 'http:' or 'https:'",
        );
      }
      if (innerRequest.method !== "GET") {
        throw new TypeError("Request method must be GET");
      }
      // Step 5.
      const innerResponse = toInnerResponse(response);
      // Step 6.
      if (innerResponse.status === 206) {
        throw new TypeError("Response status must not be 206");
      }
      // Step 7.
      const varyHeader = getHeader(innerResponse.headerList, "vary");
      if (varyHeader) {
        const fieldValues = varyHeader.split(",").map((field) => field.trim());
        for (const fieldValue of fieldValues) {
          if (
            fieldValue === "*"
          ) {
            throw new TypeError("Vary header must not contain '*'");
          }
        }
      }

      // Step 8.
      if (innerResponse.body !== null && innerResponse.body.unusable()) {
        throw new TypeError("Response body must not already used");
      }

      // Remove fragment from request URL before put.
      reqUrl.hash = "";

      // Step 9-11.
      const rid = await core.opAsync(
        "op_cache_put",
        {
          cacheId: this[_id],
          requestUrl: reqUrl.toString(),
          responseHeaders: innerResponse.headerList,
          requestHeaders: innerRequest.headerList,
          responseHasBody: innerResponse.body !== null,
          responseStatus: innerResponse.status,
          responseStatusText: innerResponse.statusMessage,
        },
      );
      if (innerResponse.body) {
        const reader = innerResponse.body.stream.getReader();
        while (true) {
          const { value, done } = await reader.read();
          if (done) {
            await core.shutdown(rid);
            core.close(rid);
            break;
          } else {
            await core.write(rid, value);
          }
        }
      }
      // Step 12-19: TODO(@satyarohith): do the insertion in background.
    }

    /** See https://w3c.github.io/ServiceWorker/#cache-match */
    async match(request, options) {
      const prefix = "Failed to execute 'match' on 'Cache'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      const p = await this.#matchAll(request, options);
      if (p.length > 0) {
        return p[0];
      } else {
        return undefined;
      }
    }

    /** See https://w3c.github.io/ServiceWorker/#cache-delete */
    async delete(request, _options) {
      const prefix = "Failed to execute 'delete' on 'Cache'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      // Step 1.
      let r = null;
      // Step 2.
      if (ObjectPrototypeIsPrototypeOf(RequestPrototype, request)) {
        r = request;
        if (request.method !== "GET") {
          return false;
        }
      } else if (
        typeof request === "string" ||
        ObjectPrototypeIsPrototypeOf(URLPrototype, request)
      ) {
        r = new Request(request);
      }
      return await core.opAsync("op_cache_delete", {
        cacheId: this[_id],
        requestUrl: r.url,
      });
    }

    /** See https://w3c.github.io/ServiceWorker/#cache-matchall
     *
     * Note: the function is private as we don't want to expose
     * this API to the public yet.
     *
     * The function will return an array of responses.
     */
    async #matchAll(request, _options) {
      // Step 1.
      let r = null;
      // Step 2.
      if (ObjectPrototypeIsPrototypeOf(RequestPrototype, request)) {
        r = request;
        if (request.method !== "GET") {
          return [];
        }
      } else if (
        typeof request === "string" ||
        ObjectPrototypeIsPrototypeOf(URLPrototype, request)
      ) {
        r = new Request(request);
      }

      // Step 5.
      const responses = [];
      // Step 5.2
      if (r === null) {
        // Step 5.3
        // Note: we have to return all responses in the cache when
        // the request is null.
        // We deviate from the spec here and return an empty array
        // as we don't expose matchAll() API.
        return responses;
      } else {
        // Remove the fragment from the request URL.
        const url = new URL(r.url);
        url.hash = "";
        const matchResult = await core.opAsync(
          "op_cache_match",
          {
            cacheId: this[_id],
            requestUrl: url.toString(),
          },
        );
        if (matchResult) {
          const [meta, responseBodyRid] = matchResult;
          let body = null;
          if (responseBodyRid !== null) {
            body = new ReadableStream({
              type: "bytes",
              async pull(controller) {
                try {
                  // This is the largest possible size for a single packet on a TLS
                  // stream.
                  const chunk = new Uint8Array(16 * 1024 + 256);
                  const read = await core.read(responseBodyRid, chunk);
                  if (read > 0) {
                    // We read some data. Enqueue it onto the stream.
                    controller.enqueue(chunk.subarray(0, read));
                  } else {
                    // We have reached the end of the body, so we close the stream.
                    core.close(responseBodyRid);
                    controller.close();
                  }
                } catch (err) {
                  // There was an error while reading a chunk of the body, so we
                  // error.
                  controller.error(err);
                  controller.close();
                  core.close(responseBodyRid);
                }
              },
            });
          }

          const response = new Response(
            body,
            {
              headers: meta.responseHeaders,
              status: meta.responseStatus,
              statusText: meta.responseStatusText,
            },
          );
          responses.push(response);
        }
      }
      // TODO(@satyarohith): Step 5.4.
      // TODO(@satyarohith): Step 5.5.

      return responses;
    }
  }

  webidl.configurePrototype(CacheStorage);
  webidl.configurePrototype(Cache);
  const CacheStoragePrototype = CacheStorage.prototype;

  let cacheStorage;
  window.__bootstrap.caches = {
    CacheStorage,
    Cache,
    cacheStorage() {
      if (!cacheStorage) {
        cacheStorage = webidl.createBranded(CacheStorage);
      }
      return cacheStorage;
    },
  };
})(this);
