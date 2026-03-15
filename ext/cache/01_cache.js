// Copyright 2018-2026 the Deno authors. MIT license.
import { primordials } from "ext:core/mod.js";
import {
  op_cache_delete,
  op_cache_match,
  op_cache_put,
  op_cache_storage_delete,
  op_cache_storage_has,
  op_cache_storage_open,
} from "ext:core/ops";
const {
  ArrayPrototypePush,
  ObjectPrototypeIsPrototypeOf,
  StringPrototypeSplit,
  StringPrototypeTrim,
  Symbol,
  SymbolFor,
  TypeError,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import {
  Request,
  RequestPrototype,
  toInnerRequest,
} from "ext:deno_fetch/23_request.js";
import { toInnerResponse } from "ext:deno_fetch/23_response.js";
import { URLPrototype } from "ext:deno_web/00_url.js";
import { getHeader } from "ext:deno_fetch/20_headers.js";
import {
  getReadableStreamResourceBacking,
  readableStreamForRid,
  resourceForReadableStream,
} from "ext:deno_web/06_streams.js";
import {
  builtinTracer,
  ContextManager,
  enterSpan,
  restoreSnapshot,
  TRACING_ENABLED,
} from "ext:deno_telemetry/telemetry.ts";

class CacheStorage {
  constructor() {
    webidl.illegalConstructor();
  }

  async open(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'open' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](cacheName, prefix, "Argument 1");

    let span;
    let snapshot;
    try {
      if (TRACING_ENABLED) {
        span = builtinTracer().startSpan(`cache open ${cacheName}`, {
          kind: 0, // INTERNAL kind
          attributes: {
            "cache.name": cacheName,
          },
        });
        snapshot = enterSpan(span);
      }

      const cacheId = await op_cache_storage_open(cacheName);
      const cache = webidl.createBranded(Cache);
      cache[_id] = cacheId;
      cache[_name] = cacheName;
      return cache;
    } catch (error) {
      if (span) {
        span.recordException(error);
        span.setStatus({ code: 2, message: error.message }); // ERROR status
      }
      throw error;
    } finally {
      if (span) span.end();
      if (snapshot) restoreSnapshot(snapshot);
    }
  }

  async has(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'has' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](cacheName, prefix, "Argument 1");

    let span;
    let snapshot;
    try {
      if (TRACING_ENABLED) {
        span = builtinTracer().startSpan(`cache has`, {
          kind: 0, // INTERNAL kind
          attributes: {
            "cache.name": cacheName,
          },
        });
        snapshot = enterSpan(span);
      }

      const result = await op_cache_storage_has(cacheName);
      span?.setAttribute("cache.exists", result);
      return result;
    } catch (error) {
      if (span) {
        span.recordException(error);
        span.setStatus({ code: 2, message: error.message }); // ERROR status
      }
      throw error;
    } finally {
      if (span) span.end();
      if (snapshot) restoreSnapshot(snapshot);
    }
  }

  async delete(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'delete' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](cacheName, prefix, "Argument 1");

    let span;
    let snapshot;
    try {
      if (TRACING_ENABLED) {
        span = builtinTracer().startSpan(`cache delete`, {
          kind: 0, // INTERNAL kind
          attributes: {
            "cache.name": cacheName,
          },
        });
        span.setAttribute("cache.name", cacheName);
        snapshot = enterSpan(span);
      }

      const result = await op_cache_storage_delete(cacheName);
      span?.setAttribute("cache.deleted", result);
      return result;
    } catch (error) {
      if (span) {
        span.recordException(error);
        span.setStatus({ code: 2, message: error.message }); // ERROR status
      }
      throw error;
    } finally {
      if (span) span.end();
      if (snapshot) restoreSnapshot(snapshot);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  }
}

const _matchAll = Symbol("[[matchAll]]");
const _id = Symbol("id");
const _name = Symbol("name");

class Cache {
  /** @type {number} */
  [_id];
  /** @type {string} */
  [_name];

  constructor() {
    webidl.illegalConstructor();
  }

  /** See https://w3c.github.io/ServiceWorker/#dom-cache-put */
  async put(request, response) {
    webidl.assertBranded(this, CachePrototype);
    const prefix = "Failed to execute 'put' on 'Cache'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    request = webidl.converters["RequestInfo_DOMString"](
      request,
      prefix,
      "Argument 1",
    );
    response = webidl.converters["Response"](response, prefix, "Argument 2");
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
        `Request url protocol must be 'http:' or 'https:': received '${reqUrl.protocol}'`,
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
      const fieldValues = StringPrototypeSplit(varyHeader, ",");
      for (let i = 0; i < fieldValues.length; ++i) {
        const field = fieldValues[i];
        if (StringPrototypeTrim(field) === "*") {
          throw new TypeError("Vary header must not contain '*'");
        }
      }
    }

    // Step 8.
    if (innerResponse.body !== null && innerResponse.body.unusable()) {
      throw new TypeError("Response body is already used");
    }

    // Remove fragment from request URL before put.
    reqUrl.hash = "";

    let span;
    let snapshot;
    try {
      if (TRACING_ENABLED) {
        // TODO: add cache-tags / deno-cache-tags
        // TODO: add response etag header
        span = builtinTracer().startSpan(`cache entry put ${this[_name]}`, {
          kind: 0,
          attributes: {
            "cache.name": this[_name],
            "url.full": reqUrl.href,
            "http.response.status": innerResponse.status,
            "http.response.body.size": innerResponse.body?.length,
          },
        }); // INTERNAL kind
        snapshot = enterSpan(span);
      }

      const stream = innerResponse.body?.stream;
      let rid = null;
      if (stream) {
        const resourceBacking = getReadableStreamResourceBacking(
          innerResponse.body?.stream,
        );
        if (resourceBacking) {
          rid = resourceBacking.rid;
        } else {
          rid = resourceForReadableStream(stream, innerResponse.body?.length);
        }
      }

      // Step 9-11.
      // Step 12-19: TODO(@satyarohith): do the insertion in background.
      await op_cache_put(
        {
          cacheId: this[_id],
          // deno-lint-ignore prefer-primordials
          requestUrl: reqUrl.toString(),
          responseHeaders: innerResponse.headerList,
          requestHeaders: innerRequest.headerList,
          responseStatus: innerResponse.status,
          responseStatusText: innerResponse.statusMessage,
          responseRid: rid,
        },
      );
    } catch (error) {
      if (span) {
        span.recordException(error);
        span.setStatus({ code: 2, message: error.message }); // ERROR status
      }
      throw error;
    } finally {
      if (span) span.end();
      if (snapshot) restoreSnapshot(snapshot);
    }
  }

  /** See https://w3c.github.io/ServiceWorker/#cache-match */
  async match(request, options) {
    webidl.assertBranded(this, CachePrototype);
    const prefix = "Failed to execute 'match' on 'Cache'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    request = webidl.converters["RequestInfo_DOMString"](
      request,
      prefix,
      "Argument 1",
    );
    const p = await this[_matchAll](request, options);
    if (p.length > 0) {
      return p[0];
    } else {
      return undefined;
    }
  }

  /** See https://w3c.github.io/ServiceWorker/#cache-delete */
  async delete(request, _options) {
    webidl.assertBranded(this, CachePrototype);
    const prefix = "Failed to execute 'delete' on 'Cache'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    request = webidl.converters["RequestInfo_DOMString"](
      request,
      prefix,
      "Argument 1",
    );

    let span;
    let snapshot;
    let r = null;

    // Step 1.
    // Step 2.
    if (ObjectPrototypeIsPrototypeOf(RequestPrototype, request)) {
      r = request;
    } else {
      r = new Request(request);
    }

    try {
      if (TRACING_ENABLED) {
        span = builtinTracer().startSpan(`cache entry delete ${this[_name]}`, {
          kind: 0, // INTERNAL kind
          attributes: {
            "cache.name": this[_name],
            "url.full": r.url,
          },
        });
        snapshot = enterSpan(span);
      }

      if (request.method !== "GET") {
        span?.setAttribute("cache.deleted", false);
        return false;
      }

      const result = await op_cache_delete({
        cacheId: this[_id],
        requestUrl: r.url,
      });
      span?.setAttribute("cache.deleted", true);
      return result;
    } catch (error) {
      if (span) {
        span.recordException(error);
        span.setStatus({ code: 2, message: error.message }); // ERROR status
      }
      throw error;
    } finally {
      if (span) span.end();
      if (snapshot) restoreSnapshot(snapshot);
    }
  }

  /** See https://w3c.github.io/ServiceWorker/#cache-matchall
   *
   * Note: the function is private as we don't want to expose
   * this API to the public yet.
   *
   * The function will return an array of responses.
   */
  async [_matchAll](request, _options) {
    let span;
    let snapshot;

    // Step 1.
    let r = null;
    // Step 2.
    if (ObjectPrototypeIsPrototypeOf(RequestPrototype, request)) {
      r = request;
    } else {
      r = new Request(request);
    }

    const url = new URL(r.url);
    // Remove the fragment from the request URL.
    url.hash = "";

    try {
      if (TRACING_ENABLED) {
        span = builtinTracer().startSpan(`cache entry match ${this[_name]}`, {
          kind: 0, // INTERNAL kind
          attributes: {
            "cache.name": this[_name],
            "url.full": url.href,
          },
        });
        snapshot = enterSpan(span);
      }

      if (request.method !== "GET") {
        span?.setAttribute("cache.matched", false);
        return [];
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
        span?.setAttribute("cache.matched", false);
        return responses;
      } else {
        const innerRequest = toInnerRequest(r);
        const matchResult = await op_cache_match(
          {
            cacheId: this[_id],
            requestUrl: url.href,
            requestHeaders: innerRequest.headerList,
          },
        );

        if (matchResult) {
          const { 0: meta, 1: responseBodyRid } = matchResult;
          let body = null;
          if (responseBodyRid !== null) {
            body = readableStreamForRid(responseBodyRid);
          }
          const response = new Response(
            body,
            {
              headers: meta.responseHeaders,
              status: meta.responseStatus,
              statusText: meta.responseStatusText,
            },
          );

          span?.setAttribute("cache.matched", true);

          ArrayPrototypePush(responses, response);
        }
      }
      // Step 5.4-5.5: don't apply in this context.

      if (span && responses.length === 0) {
        span.setAttribute("cache.matched", false);
      }

      return responses;
    } catch (error) {
      if (span) {
        span.recordException(error);
        span.setStatus({ code: 2, message: error.message }); // ERROR status
      }
      throw error;
    } finally {
      if (span) span.end();
      if (snapshot) restoreSnapshot(snapshot);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  }
}

webidl.configureInterface(CacheStorage);
webidl.configureInterface(Cache);
const CacheStoragePrototype = CacheStorage.prototype;
const CachePrototype = Cache.prototype;

let cacheStorageStorage;
function cacheStorage() {
  if (!cacheStorageStorage) {
    cacheStorageStorage = webidl.createBranded(CacheStorage);
  }
  return cacheStorageStorage;
}

export { Cache, CacheStorage, cacheStorage };
