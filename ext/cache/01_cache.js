// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  op_cache_delete,
  op_cache_match,
  op_cache_put,
  op_cache_storage_delete,
  op_cache_storage_has,
  op_cache_storage_keys,
  op_cache_storage_open,
  op_get_env_no_permission_check,
} = core.ops;
const {
  ArrayPrototypePush,
  DateNow,
  DatePrototypeGetTime,
  DatePrototypeToISOString,
  MathFloor,
  MapPrototypeGet,
  MapPrototypeSet,
  ObjectPrototypeIsPrototypeOf,
  SafeDate,
  SafeMap,
  SafeMapIterator,
  SafeSet,
  SetPrototypeAdd,
  SetPrototypeHas,
  String,
  StringPrototypeIndexOf,
  StringPrototypeSplit,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  StringPrototypeTrim,
  Symbol,
  SymbolFor,
  TypeError,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const {
  Request,
  RequestPrototype,
  toInnerRequest,
} = core.loadExtScript("ext:deno_fetch/23_request.js");
const { toInnerResponse } = core.loadExtScript(
  "ext:deno_fetch/23_response.js",
);
const { URLPrototype } = core.loadExtScript("ext:deno_web/00_url.js");
const { getHeader } = core.loadExtScript("ext:deno_fetch/20_headers.js");
const {
  getReadableStreamResourceBacking,
  readableStreamForRid,
  resourceForReadableStream,
} = core.loadExtScript("ext:deno_web/06_streams.js");

// === Backend infrastructure ===

let _backend = null;

function getBackend() {
  if (_backend !== null) return _backend;
  const lscEndpoint = op_get_env_no_permission_check(
    "DENO_CACHE_LSC_ENDPOINT",
  );
  if (lscEndpoint != null) {
    const idx = StringPrototypeIndexOf(lscEndpoint, ",");
    if (idx !== -1) {
      const endpoint = StringPrototypeSlice(lscEndpoint, 0, idx);
      const token = StringPrototypeSlice(lscEndpoint, idx + 1);
      _backend = new LscBackend(endpoint, token);
      return _backend;
    }
  }
  _backend = new SqliteRustBackend();
  return _backend;
}

// === SQLite backend (delegates to Rust ops) ===

class SqliteRustBackend {
  async storageOpen(cacheName) {
    return await op_cache_storage_open(cacheName);
  }

  async storageHas(cacheName) {
    return await op_cache_storage_has(cacheName);
  }

  async storageDelete(cacheName) {
    return await op_cache_storage_delete(cacheName);
  }

  async storageKeys() {
    return await op_cache_storage_keys();
  }

  async put(
    cacheId,
    requestUrl,
    requestHeaders,
    responseHeaders,
    responseStatus,
    responseStatusText,
    stream,
  ) {
    let rid = null;
    if (stream) {
      const resourceBacking = getReadableStreamResourceBacking(stream);
      if (resourceBacking) {
        rid = resourceBacking.rid;
      } else {
        rid = resourceForReadableStream(stream);
      }
    }
    await op_cache_put({
      cacheId,
      requestUrl,
      responseHeaders,
      requestHeaders,
      responseStatus,
      responseStatusText,
      responseRid: rid,
    });
  }

  async match(cacheId, requestUrl, requestHeaders) {
    const matchResult = await op_cache_match({
      cacheId,
      requestUrl,
      requestHeaders,
    });
    if (matchResult) {
      const { 0: meta, 1: responseBodyRid } = matchResult;
      let body = null;
      if (responseBodyRid !== null) {
        body = readableStreamForRid(responseBodyRid);
      }
      return { meta, body };
    }
    return null;
  }

  async delete(cacheId, requestUrl) {
    return await op_cache_delete({ cacheId, requestUrl });
  }
}

// === LSC backend (pure JS using fetch) ===

const REQHDR_PREFIX = "x-lsc-meta-reqhdr-";

let _internalFetch = null;
function getInternalFetch() {
  if (_internalFetch !== null) return _internalFetch;
  const mod = core.loadExtScript("ext:deno_fetch/26_fetch.js");
  _internalFetch = mod.fetch;
  return _internalFetch;
}

let _forgivingBase64UrlEncode = null;
function getBase64UrlEncode() {
  if (_forgivingBase64UrlEncode !== null) return _forgivingBase64UrlEncode;
  const mod = core.loadExtScript("ext:deno_web/00_infra.js");
  _forgivingBase64UrlEncode = mod.forgivingBase64UrlEncode;
  return _forgivingBase64UrlEncode;
}

class LscBackend {
  #endpoint;
  #token;
  #id2name = new SafeMap();
  #nextId = 0;

  constructor(endpoint, token) {
    this.#endpoint = endpoint;
    this.#token = token;
  }

  #buildObjectKey(cacheName, requestUrl) {
    const encode = getBase64UrlEncode();
    return `v1/${encode(cacheName)}/${encode(requestUrl)}`;
  }

  // deno-lint-ignore require-await
  async storageOpen(cacheName) {
    if (cacheName === "") {
      throw new TypeError("Cache name cannot be empty");
    }
    const id = this.#nextId++;
    MapPrototypeSet(this.#id2name, id, cacheName);
    return id;
  }

  // deno-lint-ignore require-await
  async storageHas(_cacheName) {
    return true;
  }

  // deno-lint-ignore require-await
  async storageDelete(_cacheName) {
    throw new TypeError("Cache deletion is not supported");
  }

  // deno-lint-ignore require-await
  async storageKeys() {
    const seen = new SafeSet();
    const names = [];
    for (const name of new SafeMapIterator(this.#id2name)) {
      if (!SetPrototypeHas(seen, name[1])) {
        SetPrototypeAdd(seen, name[1]);
        ArrayPrototypePush(names, name[1]);
      }
    }
    return names;
  }

  async put(
    cacheId,
    requestUrl,
    requestHeaders,
    responseHeaders,
    _responseStatus,
    _responseStatusText,
    bodyStream,
  ) {
    const cacheName = MapPrototypeGet(this.#id2name, cacheId);
    if (cacheName === undefined) {
      throw new TypeError("Cache not found");
    }

    const objectKey = this.#buildObjectKey(cacheName, requestUrl);
    const url = `${this.#endpoint}/objects/${objectKey}`;

    const headers = [["authorization", `Bearer ${this.#token}`]];

    // Add request headers with prefix
    for (let i = 0; i < requestHeaders.length; ++i) {
      const hdr = requestHeaders[i];
      ArrayPrototypePush(headers, [`${REQHDR_PREFIX}${hdr[0]}`, hdr[1]]);
    }

    // Add response headers (skip x-lsc-meta-* and content-encoding)
    for (let i = 0; i < responseHeaders.length; ++i) {
      const hdr = responseHeaders[i];
      const lowerName = StringPrototypeToLowerCase(hdr[0]);
      if (StringPrototypeStartsWith(lowerName, "x-lsc-meta-")) continue;
      if (lowerName === "content-encoding") {
        throw new TypeError(
          "Content-Encoding is not allowed in response headers",
        );
      }
      ArrayPrototypePush(headers, [hdr[0], hdr[1]]);
    }

    // Add cached-at timestamp (seconds precision, Z suffix)
    const now = new SafeDate(DateNow());
    const isoStr = DatePrototypeToISOString(now);
    // Strip milliseconds: "2024-01-01T00:00:00.000Z" -> "2024-01-01T00:00:00Z"
    const cachedAt = StringPrototypeSlice(isoStr, 0, 19) + "Z";
    ArrayPrototypePush(headers, ["x-lsc-meta-cached-at", cachedAt]);

    const fetch = getInternalFetch();
    const resp = await fetch(url, {
      method: "PUT",
      headers,
      body: bodyStream || null,
    });

    if (!resp.ok) {
      throw new TypeError(`cache PUT request failed: ${resp.status}`);
    }
  }

  async match(cacheId, requestUrl, requestHeaders) {
    const cacheName = MapPrototypeGet(this.#id2name, cacheId);
    if (cacheName === undefined) {
      throw new TypeError("Cache not found");
    }

    const objectKey = this.#buildObjectKey(cacheName, requestUrl);
    const url = `${this.#endpoint}/objects/${objectKey}`;

    const fetch = getInternalFetch();
    const resp = await fetch(url, {
      method: "GET",
      headers: [
        ["authorization", `Bearer ${this.#token}`],
        ["x-ryw", "1"],
      ],
    });

    if (resp.status === 404) return null;
    if (!resp.ok) {
      throw new TypeError(`cache GET request failed: ${resp.status}`);
    }

    // Check for tombstone
    if (resp.headers.has("x-lsc-meta-deleted-at")) return null;

    // Vary header matching
    const varyHeader = resp.headers.get("vary");
    if (varyHeader !== null) {
      if (!this.#varyMatches(varyHeader, requestHeaders, resp.headers)) {
        return null;
      }
    }

    // Build response headers (filter out x-lsc-meta-* and x-ryw)
    const responseHeaders = [];
    // deno-lint-ignore prefer-primordials
    for (const [name, value] of resp.headers) {
      if (
        StringPrototypeStartsWith(name, "x-lsc-meta-") || name === "x-ryw"
      ) {
        continue;
      }
      ArrayPrototypePush(responseHeaders, [name, value]);
    }

    // Compute age from x-lsc-meta-cached-at
    const cachedAtStr = resp.headers.get("x-lsc-meta-cached-at");
    if (cachedAtStr !== null) {
      const cachedDate = new SafeDate(cachedAtStr);
      const cachedTime = DatePrototypeGetTime(cachedDate);
      if (cachedTime === cachedTime) { // not NaN
        const ageSecs = MathFloor((DateNow() - cachedTime) / 1000);
        if (ageSecs >= 0) {
          ArrayPrototypePush(responseHeaders, ["age", String(ageSecs)]);
        }
      }
    }

    // Build request headers from x-lsc-meta-reqhdr-* prefixed headers
    const cachedRequestHeaders = [];
    // deno-lint-ignore prefer-primordials
    for (const [name, value] of resp.headers) {
      if (StringPrototypeStartsWith(name, REQHDR_PREFIX)) {
        ArrayPrototypePush(cachedRequestHeaders, [
          StringPrototypeSlice(name, REQHDR_PREFIX.length),
          value,
        ]);
      }
    }

    const meta = {
      responseStatus: resp.status,
      responseStatusText: resp.statusText,
      requestHeaders: cachedRequestHeaders,
      responseHeaders,
    };

    return { meta, body: resp.body };
  }

  async delete(cacheId, requestUrl) {
    const cacheName = MapPrototypeGet(this.#id2name, cacheId);
    if (cacheName === undefined) {
      throw new TypeError("Cache not found");
    }

    const objectKey = this.#buildObjectKey(cacheName, requestUrl);
    const url = `${this.#endpoint}/objects/${objectKey}`;

    // Add deleted-at timestamp
    const now = new SafeDate(DateNow());
    const isoStr = DatePrototypeToISOString(now);
    const deletedAt = StringPrototypeSlice(isoStr, 0, 19) + "Z";

    const fetch = getInternalFetch();
    const resp = await fetch(url, {
      method: "PUT",
      headers: [
        ["authorization", `Bearer ${this.#token}`],
        ["expires", "Thu, 01 Jan 1970 00:00:00 GMT"],
        ["x-lsc-meta-deleted-at", deletedAt],
      ],
      body: null,
    });

    if (!resp.ok) {
      throw new TypeError(`cache DELETE request failed: ${resp.status}`);
    }
    return true;
  }

  #varyMatches(varyHeader, queryRequestHeaders, cachedHeaders) {
    const fields = StringPrototypeSplit(varyHeader, ",");
    for (let i = 0; i < fields.length; ++i) {
      const field = StringPrototypeToLowerCase(
        StringPrototypeTrim(fields[i]),
      );
      // Ignoring accept-encoding is safe because we refuse to cache
      // responses with content-encoding
      if (field === "accept-encoding") continue;

      const lookupKey = `${REQHDR_PREFIX}${field}`;
      const queryValue = this.#getHeaderValue(field, queryRequestHeaders);
      const cachedValue = cachedHeaders.get(lookupKey);
      if (queryValue !== cachedValue) return false;
    }
    return true;
  }

  #getHeaderValue(name, headers) {
    for (let i = 0; i < headers.length; ++i) {
      const hdr = headers[i];
      if (StringPrototypeToLowerCase(hdr[0]) === name) return hdr[1];
    }
    return null;
  }
}

// === Web Cache API classes ===

class CacheStorage {
  constructor() {
    webidl.illegalConstructor();
  }

  async open(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'open' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](
      cacheName,
      prefix,
      "Argument 1",
    );
    const backend = getBackend();
    const cacheId = await backend.storageOpen(cacheName);
    const cache = webidl.createBranded(Cache);
    cache[_id] = cacheId;
    return cache;
  }

  async has(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'has' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](
      cacheName,
      prefix,
      "Argument 1",
    );
    const backend = getBackend();
    return await backend.storageHas(cacheName);
  }

  async delete(cacheName) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'delete' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    cacheName = webidl.converters["DOMString"](
      cacheName,
      prefix,
      "Argument 1",
    );
    const backend = getBackend();
    return await backend.storageDelete(cacheName);
  }

  async keys() {
    webidl.assertBranded(this, CacheStoragePrototype);
    const backend = getBackend();
    return await backend.storageKeys();
  }

  async match(request, options) {
    webidl.assertBranded(this, CacheStoragePrototype);
    const prefix = "Failed to execute 'match' on 'CacheStorage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    request = webidl.converters["RequestInfo_DOMString"](
      request,
      prefix,
      "Argument 1",
    );
    const backend = getBackend();
    const cacheName = options?.cacheName;
    if (cacheName !== undefined) {
      if (!(await backend.storageHas(cacheName))) {
        return undefined;
      }
      const cache = await this.open(cacheName);
      // false positive: cache is a local Cache instance, not a global intrinsic
      // deno-lint-ignore prefer-primordials
      return await cache.match(request, options);
    }
    const names = await backend.storageKeys();
    for (let i = 0; i < names.length; ++i) {
      const cache = await this.open(names[i]);
      // false positive: cache is a local Cache instance, not a global intrinsic
      // deno-lint-ignore prefer-primordials
      const response = await cache.match(request, options);
      if (response !== undefined) {
        return response;
      }
    }
    return undefined;
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

    // Remove fragment from request URL before put.
    reqUrl.hash = "";

    const backend = getBackend();
    // Step 9-11.
    // Step 12-19: TODO(@satyarohith): do the insertion in background.
    await backend.put(
      this[_id],
      // deno-lint-ignore prefer-primordials
      reqUrl.toString(),
      innerRequest.headerList,
      innerResponse.headerList,
      innerResponse.status,
      innerResponse.statusMessage,
      stream || null,
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
    const backend = getBackend();
    return await backend.delete(this[_id], r.url);
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
      const backend = getBackend();
      // false positive: backend is a local class instance, not a global intrinsic
      // deno-lint-ignore prefer-primordials
      const matchResult = await backend.match(
        this[_id],
        // deno-lint-ignore prefer-primordials
        url.toString(),
        innerRequest.headerList,
      );
      if (matchResult) {
        const { meta, body } = matchResult;
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

return { Cache, CacheStorage, cacheStorage };
})();
