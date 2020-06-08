// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { notImplemented } from "../util.ts";
import { isTypedArray } from "./util.ts";
import * as domTypes from "./dom_types.d.ts";
import { TextEncoder } from "./text_encoding.ts";
import { DenoBlob, bytesSymbol as blobBytesSymbol } from "./blob.ts";
import { read } from "../ops/io.ts";
import { close } from "../ops/resources.ts";
import { fetch as opFetch, FetchResponse } from "../ops/fetch.ts";
import * as Body from "./body.ts";
import { getHeaderValueParams } from "./util.ts";
import { ReadableStreamImpl } from "./streams/readable_stream.ts";
import { MultipartBuilder } from "./fetch/multipart.ts";

const NULL_BODY_STATUS = [101, 204, 205, 304];
const REDIRECT_STATUS = [301, 302, 303, 307, 308];

const responseData = new WeakMap();
export class Response extends Body.Body implements domTypes.Response {
  readonly type: ResponseType;
  readonly redirected: boolean;
  readonly url: string;
  readonly status: number;
  readonly statusText: string;
  headers: Headers;

  constructor(body: BodyInit | null = null, init?: domTypes.ResponseInit) {
    init = init ?? {};

    if (typeof init !== "object") {
      throw new TypeError(`'init' is not an object`);
    }

    const extraInit = responseData.get(init) || {};
    let { type = "default", url = "" } = extraInit;

    let status = (Number(init.status) || 0) ?? 200;
    let statusText = init.statusText ?? "";
    let headers =
      init.headers instanceof Headers
        ? init.headers
        : new Headers(init.headers);

    if (init.status && (status < 200 || status > 599)) {
      throw new RangeError(
        `The status provided (${init.status}) is outside the range [200, 599]`
      );
    }

    // null body status
    if (body && NULL_BODY_STATUS.includes(status)) {
      throw new TypeError("Response with null body status cannot have body");
    }

    if (!type) {
      type = "default";
    } else {
      type = type;
      if (type == "error") {
        // spec: https://fetch.spec.whatwg.org/#concept-network-error
        status = 0;
        statusText = "";
        headers = new Headers();
        body = null;
        /* spec for other Response types:
           https://fetch.spec.whatwg.org/#concept-filtered-response-basic
           Please note that type "basic" is not the same thing as "default".*/
      } else if (type == "basic") {
        for (const h of headers) {
          /* Forbidden Response-Header Names:
             https://fetch.spec.whatwg.org/#forbidden-response-header-name */
          if (["set-cookie", "set-cookie2"].includes(h[0].toLowerCase())) {
            headers.delete(h[0]);
          }
        }
      } else if (type == "cors") {
        /* CORS-safelisted Response-Header Names:
             https://fetch.spec.whatwg.org/#cors-safelisted-response-header-name */
        const allowedHeaders = [
          "Cache-Control",
          "Content-Language",
          "Content-Length",
          "Content-Type",
          "Expires",
          "Last-Modified",
          "Pragma",
        ].map((c: string) => c.toLowerCase());
        for (const h of headers) {
          /* Technically this is still not standards compliant because we are
             supposed to allow headers allowed in the
             'Access-Control-Expose-Headers' header in the 'internal response'
             However, this implementation of response doesn't seem to have an
             easy way to access the internal response, so we ignore that
             header.
             TODO(serverhiccups): change how internal responses are handled
             so we can do this properly. */
          if (!allowedHeaders.includes(h[0].toLowerCase())) {
            headers.delete(h[0]);
          }
        }
        /* TODO(serverhiccups): Once I fix the 'internal response' thing,
           these actually need to treat the internal response differently */
      } else if (type == "opaque" || type == "opaqueredirect") {
        url = "";
        status = 0;
        statusText = "";
        headers = new Headers();
        body = null;
      }
    }

    const contentType = headers.get("content-type") || "";

    super(body, contentType);

    this.url = url;
    this.statusText = statusText;
    this.status = extraInit.status || status;
    this.headers = headers;
    this.redirected = extraInit.redirected;
    this.type = type;
  }

  get ok(): boolean {
    return 200 <= this.status && this.status < 300;
  }

  public clone(): domTypes.Response {
    if (this.bodyUsed) {
      throw TypeError(Body.BodyUsedError);
    }

    const iterators = this.headers.entries();
    const headersList: Array<[string, string]> = [];
    for (const header of iterators) {
      headersList.push(header);
    }

    let resBody = this._bodySource;

    if (this._bodySource instanceof ReadableStreamImpl) {
      const tees = this._bodySource.tee();
      this._stream = this._bodySource = tees[0];
      resBody = tees[1];
    }

    const cloned = new Response(resBody, {
      status: this.status,
      statusText: this.statusText,
      headers: new Headers(headersList),
    });
    return cloned;
  }

  static redirect(url: URL | string, status: number): domTypes.Response {
    if (![301, 302, 303, 307, 308].includes(status)) {
      throw new RangeError(
        "The redirection status must be one of 301, 302, 303, 307 and 308."
      );
    }
    return new Response(null, {
      status,
      statusText: "",
      headers: [["Location", typeof url === "string" ? url : url.toString()]],
    });
  }
}

function sendFetchReq(
  url: string,
  method: string | null,
  headers: Headers | null,
  body: ArrayBufferView | undefined
): Promise<FetchResponse> {
  let headerArray: Array<[string, string]> = [];
  if (headers) {
    headerArray = Array.from(headers.entries());
  }

  const args = {
    method,
    url,
    headers: headerArray,
  };

  return opFetch(args, body);
}

export async function fetch(
  input: (domTypes.Request & { _bodySource?: unknown }) | URL | string,
  init?: domTypes.RequestInit
): Promise<Response> {
  let url: string;
  let method: string | null = null;
  let headers: Headers | null = null;
  let body: ArrayBufferView | undefined;
  let redirected = false;
  let remRedirectCount = 20; // TODO: use a better way to handle

  if (typeof input === "string" || input instanceof URL) {
    url = typeof input === "string" ? (input as string) : (input as URL).href;
    if (init != null) {
      method = init.method || null;
      if (init.headers) {
        headers =
          init.headers instanceof Headers
            ? init.headers
            : new Headers(init.headers);
      } else {
        headers = null;
      }

      // ref: https://fetch.spec.whatwg.org/#body-mixin
      // Body should have been a mixin
      // but we are treating it as a separate class
      if (init.body) {
        if (!headers) {
          headers = new Headers();
        }
        let contentType = "";
        if (typeof init.body === "string") {
          body = new TextEncoder().encode(init.body);
          contentType = "text/plain;charset=UTF-8";
        } else if (isTypedArray(init.body)) {
          body = init.body;
        } else if (init.body instanceof ArrayBuffer) {
          body = new Uint8Array(init.body);
        } else if (init.body instanceof URLSearchParams) {
          body = new TextEncoder().encode(init.body.toString());
          contentType = "application/x-www-form-urlencoded;charset=UTF-8";
        } else if (init.body instanceof DenoBlob) {
          body = init.body[blobBytesSymbol];
          contentType = init.body.type;
        } else if (init.body instanceof FormData) {
          let boundary;
          if (headers.has("content-type")) {
            const params = getHeaderValueParams("content-type");
            boundary = params.get("boundary")!;
          }
          const multipartBuilder = new MultipartBuilder(init.body, boundary);
          body = multipartBuilder.getBody();
          contentType = multipartBuilder.getContentType();
        } else {
          // TODO: ReadableStream
          notImplemented();
        }
        if (contentType && !headers.has("content-type")) {
          headers.set("content-type", contentType);
        }
      }
    }
  } else {
    url = input.url;
    method = input.method;
    headers = input.headers;

    if (input._bodySource) {
      body = new DataView(await input.arrayBuffer());
    }
  }

  let responseBody;
  let responseInit: ResponseInit = {};
  while (remRedirectCount) {
    const fetchResponse = await sendFetchReq(url, method, headers, body);

    if (
      NULL_BODY_STATUS.includes(fetchResponse.status) ||
      REDIRECT_STATUS.includes(fetchResponse.status)
    ) {
      // We won't use body of received response, so close it now
      // otherwise it will be kept in resource table.
      close(fetchResponse.bodyRid);
      responseBody = null;
    } else {
      responseBody = new ReadableStreamImpl({
        async pull(controller: ReadableStreamDefaultController): Promise<void> {
          try {
            const b = new Uint8Array(1024 * 32);
            const result = await read(fetchResponse.bodyRid, b);
            if (result === null) {
              controller.close();
              return close(fetchResponse.bodyRid);
            }

            controller.enqueue(b.subarray(0, result));
          } catch (e) {
            controller.error(e);
            controller.close();
            close(fetchResponse.bodyRid);
          }
        },
        cancel(): void {
          // When reader.cancel() is called
          close(fetchResponse.bodyRid);
        },
      });
    }

    responseInit = {
      status: 200,
      statusText: fetchResponse.statusText,
      headers: fetchResponse.headers,
    };

    responseData.set(responseInit, {
      redirected,
      rid: fetchResponse.bodyRid,
      status: fetchResponse.status,
      url,
    });

    const response = new Response(responseBody, responseInit);

    if (REDIRECT_STATUS.includes(fetchResponse.status)) {
      // We're in a redirect status
      switch ((init && init.redirect) || "follow") {
        case "error":
          responseInit = {};
          responseData.set(responseInit, {
            type: "error",
            redirected: false,
            url: "",
          });
          return new Response(null, responseInit);
        case "manual":
          responseInit = {};
          responseData.set(responseInit, {
            type: "opaqueredirect",
            redirected: false,
            url: "",
          });
          return new Response(null, responseInit);
        case "follow":
        default:
          let redirectUrl = response.headers.get("Location");
          if (redirectUrl == null) {
            return response; // Unspecified
          }
          if (
            !redirectUrl.startsWith("http://") &&
            !redirectUrl.startsWith("https://")
          ) {
            redirectUrl =
              url.split("//")[0] +
              "//" +
              url.split("//")[1].split("/")[0] +
              redirectUrl; // TODO: handle relative redirection more gracefully
          }
          url = redirectUrl;
          redirected = true;
          remRedirectCount--;
      }
    } else {
      return response;
    }
  }

  responseData.set(responseInit, {
    type: "error",
    redirected: false,
    url: "",
  });

  return new Response(null, responseInit);
}
