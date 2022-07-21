// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.__bootstrap.core;
  const {
    ArrayPrototypeFrom,
    PromiseResolve,
    PromiseReject,
    TypeError,
    Uint8Array,
  } = window.__bootstrap.primordials;

  class CacheStorage {
    constructor() {
      return this;
    }

    async open(cacheName) {
      try {
        const cacheId = await core.opAsync("op_cache_storage_open", cacheName);
        return PromiseResolve(new Cache(cacheId));
      } catch (error) {
        return PromiseReject(error);
      }
    }

    async delete(cacheName) {
      try {
        return await core.opAsync("op_cache_storage_delete", cacheName);
      } catch (error) {
        return PromiseReject(error);
      }
    }

    async has(cacheName) {
      try {
        return await core.opAsync("op_cache_storage_has", cacheName);
      } catch (error) {
        return PromiseReject(error);
      }
    }
  }

  class Cache {
    #id;

    constructor(cacheId) {
      this.#id = cacheId;
      return this;
    }

    /** See https://w3c.github.io/ServiceWorker/#dom-cache-put */
    async put(request, response) {
      // Step 1.
      let innerRequest = null;
      // Step 2.
      if (request instanceof Request) {
        innerRequest = request;
      } else {
        // Step 3.
        try {
          innerRequest = new Request(request);
        } catch (error) {
          return PromiseReject(error);
        }
      }
      // Step 4.
      const url = new URL(innerRequest.url);
      if (url.protocol !== "http:" && url.protocol !== "https:") {
        return PromiseReject(
          new TypeError("Request url protocol must be http or https"),
        );
      }
      if (innerRequest.method !== "GET") {
        return PromiseReject(new TypeError("Request method must be GET"));
      }
      // Step 5.
      const innerResponse = response;
      // Step 6.
      if (innerResponse.status === 206) {
        return PromiseReject(
          new TypeError("Response status must not be 206"),
        );
      }
      // Step 7.
      const varyHeader = innerResponse.headers.get("Vary");
      if (varyHeader) {
        const fieldValues = varyHeader.split(",").map((field) => field.trim());
        for (const fieldValue of fieldValues) {
          if (
            fieldValue === "*"
          ) {
            return PromiseReject(
              new TypeError("Vary header must not contain '*'"),
            );
          }
        }
      }

      // Step 8.
      // TODO(@satyarohith): step 8 mentions innerResponse body is disturbed.
      // How to check body is disturbed? I'm using bodyUsed flag.
      if (innerResponse.bodyUsed) {
        return PromiseReject(
          new TypeError("Response body must not already used"),
        );
      }

      // Step 9.
      // Step 10.
      // Step 11.
      const rid = await core.opAsync(
        "op_cache_put",
        {
          cacheId: this.#id,
          requestUrl: innerRequest.url,
          responseHeaders: ArrayPrototypeFrom(innerResponse.headers.entries()),
          requestHeaders: ArrayPrototypeFrom(innerRequest.headers.entries()),
          responseHasBody: innerResponse.body !== null,
          responseStatus: innerResponse.status,
          responseStatusText: innerResponse.statusText,
        },
      );
      if (innerResponse.body) {
        const reader = innerResponse.body.getReader();
        while (true) {
          const { value, done } = await reader.read();
          if (done) {
            await core.shutdown(rid);
            break;
          } else {
            await core.write(rid, value);
          }
        }
      }
      // TODO(@satyarohith): step 12-19.
    }

    /** See https://w3c.github.io/ServiceWorker/#cache-match */
    async match(request, options) {
      try {
        const p = await this.#matchAll(request, options);
        if (p.length > 0) {
          return PromiseResolve(p[0]);
        } else {
          return PromiseResolve(undefined);
        }
      } catch (error) {
        return PromiseReject(error);
      }
    }

    async delete(request, _options) {
      return await core.opAsync("op_cache_delete", {
        cacheId: this.#id,
        requestUrl: request.url,
      });
    }

    /** See https://w3c.github.io/ServiceWorker/#cache-matchall
     * Note: the function is private as we don't want to expose
     * this API to the public yet.
     *
     * The function will return an array of responses.
     */
    async #matchAll(request, options = {}) {
      // Step 1.
      let r = null;
      // Step 2.
      if (request instanceof Request) {
        r = request;
        if (request.method !== "GET" && !options["ignoreMethod"]) {
          return PromiseResolve([]);
        }
      } else if (request instanceof string) {
        try {
          r = new Request(request);
        } catch (error) {
          return PromiseReject(error);
        }
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
        const result = await core.opAsync("op_cache_match", {
          cacheId: this.#id,
          requestUrl: r.url,
          requestHeaders: ArrayPrototypeFrom(r.headers.entries()),
        });
        if (result) {
          let body = null;
          if (result.responseBodyRid !== null) {
            body = new ReadableStream({
              type: "bytes",
              async pull(controller) {
                try {
                  // This is the largest possible size for a single packet on a TLS
                  // stream.
                  const chunk = new Uint8Array(16 * 1024 + 256);
                  const read = await core.read(result.responseBodyRid, chunk);
                  if (read > 0) {
                    // We read some data. Enqueue it onto the stream.
                    controller.enqueue(chunk.subarray(0, read));
                  } else {
                    // We have reached the end of the body, so we close the stream.
                    controller.close();
                  }
                } catch (err) {
                  // There was an error while reading a chunk of the body, so we
                  // error.
                  controller.error(err);
                  controller.close();
                }
              },
            });
          }

          const response = new Response(
            body,
            {
              headers: result.responseHeaders,
              status: result.responseStatus,
              statusText: result.responseStatusText,
            },
          );
          responses.push(response);
        }
      }
      // TODO(@satyarohith): Step 5.4.
      // TODO(@satyarohith): Step 5.5.

      return PromiseResolve(responses);
    }

    // /** Query cache for the provided request.
    //  * See https://w3c.github.io/ServiceWorker/#query-cache-algorithm. */
    // #queryCache(requestQuery, options = {
    //   ignoreMethod: false,
    //   ignoreSearch: false,
    //   ignoreVary: false,
    // }, targetStorage) {
    //   // Step 1.
    //   const resultList = new Map();
    //   // Step 2.
    //   let storage = null;
    //   if (!targetStorage) {
    //     // Step 3.
    //     // storage = this.#storage;
    //     storage = new Map();
    //   } else {
    //     // Step 4.
    //     storage = targetStorage;
    //   }

    //   // Step 5.
    //   for (const [request, response] of storage.entries()) {
    //     const cachedRequest = request;
    //     const cachedResponse = response;
    //     const matchesCachedItem = requestMatchesCatchedItem(
    //       requestQuery,
    //       cachedRequest,
    //       cachedResponse,
    //       options,
    //     );
    //     if (matchesCachedItem) {
    //       resultList.set(request, response);
    //     }
    //   }
    //   return resultList;
    // }
  }

  // /** See https://w3c.github.io/ServiceWorker/#request-matches-cached-item-algorithm */
  // function requestMatchesCatchedItem(
  //   requestQuery,
  //   request,
  //   response = null,
  //   options = {
  //     ignoreMethod: false,
  //     ignoreSearch: false,
  //     ignoreVary: false,
  //   },
  // ) {
  //   // Step 1.
  //   if (options["ignoreMethod"] === false && request.method !== "GET") {
  //     return false;
  //   }

  //   // Step 2.
  //   const queryURL = new URL(requestQuery.url);
  //   // Step 3.
  //   const cachedURL = new URL(request.url);
  //   // Step 4.
  //   if (options["ignoreSearch"] === true) {
  //     queryURL.search = "";
  //     cachedURL.search = "";
  //   }

  //   // Step 5.
  //   {
  //     const a = new URL(queryURL);
  //     const b = new URL(cachedURL);
  //     // Note: interpreting `exclude fragment flag` from spec as don't
  //     // compare a.hash !== b.hash.
  //     if (
  //       a.origin !== b.origin ||
  //       a.pathname !== b.pathname ||
  //       a.search !== b.search
  //     ) {
  //       return false;
  //     }
  //   }

  //   // Step 6.
  //   if (
  //     response === null || options["ignoreVary"] === true ||
  //     !response.headers.has("Vary")
  //   ) {
  //     return true;
  //   }

  //   // Step 7.
  //   const varyHeader = response.headers.get("Vary");
  //   const fieldValues = varyHeader.split(",").map((field) => field.trim());
  //   // TODO(@satyarohith):
  //   // If fieldValue matches "*", or the combined value given fieldValue
  //   // and request’s header list does not match the combined value given
  //   // fieldValue and requestQuery’s header list, then return false.
  //   for (const fieldValue of fieldValues) {
  //     if (
  //       fieldValue === "*" ||
  //       request.headers.get("Vary") !== requestQuery.headers.get("Vary")
  //     ) {
  //       return false;
  //     }
  //   }

  //   return true;
  // }

  window.__bootstrap.caches = {
    CacheStorage,
  };
})(this);
