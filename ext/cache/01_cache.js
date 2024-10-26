// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
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
import { URLPrototype } from "ext:deno_url/00_url.js";
import { getHeader } from "ext:deno_fetch/20_headers.js";
import {
  getReadableStreamResourceBacking,
  readableStreamForRid,
  resourceForReadableStream,
} from "ext:deno_web/06_streams.js";

class CacheStorage {
  constructor() {
    webidl.illegalConstructor();
  }

  async open(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'open' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](cacheName, prefix, "Argument 1");
    const cacheId = await op_cache_storage_open(cacheName);
    const cache = webidl.createBranded(Cache);
    cache[_id] = cacheId;
    return cache;
  }

  async has(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'has' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](cacheName, prefix, "Argument 1");
    return await op_cache_storage_has(cacheName);
  }

  async delete(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'delete' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](cacheName, prefix, "Argument 1");
    return await op_cache_storage_delete(cacheName);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  }
}

const _matchAll = Symbol("[[matchAll]]");
const _id = Symbol("id");

class Cache {
  /** @type {number} */
  [_id];

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

    // Remove fragment from request URL before put.
    reqUrl.hash = "";

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
    return await op_cache_delete({
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
  async [_matchAll](request, _options) {
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
      const innerRequest = toInnerRequest(r);
      const matchResult = await op_cache_match(
        {
          cacheId: this[_id],
          // deno-lint-ignore prefer-primordials
          requestUrl: url.toString(),
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
        ArrayPrototypePush(responses, response);
      }
    }
    // Step 5.4-5.5: don't apply in this context.

    return responses;
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
