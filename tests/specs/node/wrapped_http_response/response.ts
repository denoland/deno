// Adapted from https://github.com/honojs/node-server/blob/1eb73c6d985665e75458ddd08c23bbc1dbdc7bcd/src/response.ts
// deno-lint-ignore-file no-explicit-any
//
import type { OutgoingHttpHeaders } from "node:http";

interface InternalBody {
  source: string | Uint8Array | FormData | Blob | null;
  stream: ReadableStream;
  length: number | null;
}

const GlobalResponse = globalThis.Response;

const responseCache = Symbol("responseCache");
const getResponseCache = Symbol("getResponseCache");
export const cacheKey = Symbol("cache");

export const buildOutgoingHttpHeaders = (
  headers: Headers | HeadersInit | null | undefined,
): OutgoingHttpHeaders => {
  const res: OutgoingHttpHeaders = {};
  if (!(headers instanceof Headers)) {
    headers = new Headers(headers ?? undefined);
  }

  const cookies = [];
  for (const [k, v] of headers) {
    if (k === "set-cookie") {
      cookies.push(v);
    } else {
      res[k] = v;
    }
  }
  if (cookies.length > 0) {
    res["set-cookie"] = cookies;
  }
  res["content-type"] ??= "text/plain; charset=UTF-8";

  return res;
};

export class Response {
  #body?: BodyInit | null;
  #init?: ResponseInit;

  [getResponseCache](): typeof GlobalResponse {
    delete (this as any)[cacheKey];
    return ((this as any)[responseCache] ||= new GlobalResponse(
      this.#body,
      this.#init,
    ));
  }

  constructor(body?: BodyInit | null, init?: ResponseInit) {
    this.#body = body;
    if (init instanceof Response) {
      const cachedGlobalResponse = (init as any)[responseCache];
      if (cachedGlobalResponse) {
        this.#init = cachedGlobalResponse;
        // instantiate GlobalResponse cache and this object always returns value from global.Response
        this[getResponseCache]();
        return;
      } else {
        this.#init = init.#init;
      }
    } else {
      this.#init = init;
    }

    if (
      typeof body === "string" ||
      typeof (body as ReadableStream)?.getReader !== "undefined"
    ) {
      let headers =
        (init?.headers || { "content-type": "text/plain; charset=UTF-8" }) as
          | Record<string, string>
          | Headers
          | OutgoingHttpHeaders;
      if (headers instanceof Headers) {
        headers = buildOutgoingHttpHeaders(headers);
      }

      (this as any)[cacheKey] = [init?.status || 200, body, headers];
    }
  }
}
[
  "body",
  "bodyUsed",
  "headers",
  "ok",
  "redirected",
  "status",
  "statusText",
  "trailers",
  "type",
  "url",
].forEach((k) => {
  Object.defineProperty(Response.prototype, k, {
    get() {
      return this[getResponseCache]()[k];
    },
  });
});
["arrayBuffer", "blob", "clone", "formData", "json", "text"].forEach((k) => {
  Object.defineProperty(Response.prototype, k, {
    value: function () {
      return this[getResponseCache]()[k]();
    },
  });
});
Object.setPrototypeOf(Response, GlobalResponse);
Object.setPrototypeOf(Response.prototype, GlobalResponse.prototype);
