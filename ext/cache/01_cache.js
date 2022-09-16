// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.__bootstrap.core;
  const {
    PromiseResolve,
    PromiseReject,
    TypeError,
    Uint8Array,
  } = window.__bootstrap.primordials;
  const {
    toInnerResponse,
    toInnerRequest,
  } = window.__bootstrap.fetch;
  const { getHeader } = window.__bootstrap.headers;

  class CacheStorage {
    constructor() {
      return this;
    }

    async open(cacheName) {
      const cacheId = await core.opAsync("op_cache_storage_open", cacheName);
      return PromiseResolve(new Cache(cacheId));
    }

    async has(cacheName) {
      return await core.opAsync("op_cache_storage_has", cacheName);
    }

    async delete(cacheName) {
      return await core.opAsync("op_cache_storage_delete", cacheName);
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
        innerRequest = toInnerRequest(request);
      } else {
        // Step 3.
        try {
          innerRequest = toInnerRequest(new Request(request));
        } catch (error) {
          return PromiseReject(error);
        }
      }
      // Step 4.
      const reqUrl = innerRequest.url();
      if (!reqUrl.startsWith("http:") && !reqUrl.startsWith("https:")) {
        const url = new URL(reqUrl);
        if (url.protocol !== "http:" && url.protocol !== "https:") {
          return PromiseReject(
            new TypeError("Request url protocol must be http or https"),
          );
        }
      }
      if (innerRequest.method !== "GET") {
        return PromiseReject(new TypeError("Request method must be GET"));
      }
      // Step 5.
      const innerResponse = toInnerResponse(response);
      // Step 6.
      if (innerResponse.status === 206) {
        return PromiseReject(
          new TypeError("Response status must not be 206"),
        );
      }
      // Step 7.
      const varyHeader = getHeader(innerResponse.headerList, "vary");
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
      if (innerResponse.body.unusable()) {
        return PromiseReject(
          new TypeError("Response body must not already used"),
        );
      }

      // Step 9-11.
      const rid = await core.opAsync(
        "op_cache_put",
        {
          cacheId: this.#id,
          requestUrl: innerRequest.url(),
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

    async delete(request, options) {
      let r = null;
      // Step 2.
      if (request instanceof Request) {
        r = request;
        if (request.method !== "GET" && !options["ignoreMethod"]) {
          return PromiseResolve([]);
        }
      } else if (typeof request === "string" || request instanceof URL) {
        try {
          r = new Request(request);
        } catch (error) {
          return PromiseReject(error);
        }
      }
      return await core.opAsync("op_cache_delete", {
        cacheId: this.#id,
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
    async #matchAll(request, options = {}) {
      // Step 1.
      let r = null;
      // Step 2.
      if (request instanceof Request) {
        r = request;
        if (request.method !== "GET" && !options["ignoreMethod"]) {
          return PromiseResolve([]);
        }
      } else if (typeof request === "string" || request instanceof URL) {
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
        const matchResult = await core.opAsync(
          "op_cache_match",
          {
            cacheId: this.#id,
            requestUrl: r.url,
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

      return PromiseResolve(responses);
    }
  }

  window.__bootstrap.caches = {
    CacheStorage,
  };
})(this);
