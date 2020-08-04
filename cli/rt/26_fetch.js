// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { notImplemented } = window.__bootstrap.util;
  const { getHeaderValueParams, isTypedArray } = window.__bootstrap.webUtil;
  const { Blob, bytesSymbol: blobBytesSymbol } = window.__bootstrap.blob;
  const { read } = window.__bootstrap.io;
  const { close } = window.__bootstrap.resources;
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;
  const Body = window.__bootstrap.body;
  const { ReadableStream } = window.__bootstrap.streams;
  const { MultipartBuilder } = window.__bootstrap.multipart;
  const { Headers } = window.__bootstrap.headers;

  function createHttpClient(options) {
    return new HttpClient(opCreateHttpClient(options));
  }

  function opCreateHttpClient(args) {
    return sendSync("op_create_http_client", args);
  }

  class HttpClient {
    constructor(rid) {
      this.rid = rid;
    }
    close() {
      close(this.rid);
    }
  }

  function opFetch(args, body) {
    let zeroCopy;
    if (body != null) {
      zeroCopy = new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
    }

    return sendAsync("op_fetch", args, ...(zeroCopy ? [zeroCopy] : []));
  }

  const NULL_BODY_STATUS = [101, 204, 205, 304];
  const REDIRECT_STATUS = [301, 302, 303, 307, 308];

  const responseData = new WeakMap();
  class Response extends Body.Body {
    constructor(body = null, init) {
      init = init ?? {};

      if (typeof init !== "object") {
        throw new TypeError(`'init' is not an object`);
      }

      const extraInit = responseData.get(init) || {};
      let { type = "default", url = "" } = extraInit;

      let status = init.status === undefined ? 200 : Number(init.status || 0);
      let statusText = init.statusText ?? "";
      let headers = init.headers instanceof Headers
        ? init.headers
        : new Headers(init.headers);

      if (init.status !== undefined && (status < 200 || status > 599)) {
        throw new RangeError(
          `The status provided (${init.status}) is outside the range [200, 599]`,
        );
      }

      // null body status
      if (body && NULL_BODY_STATUS.includes(status)) {
        throw new TypeError("Response with null body status cannot have body");
      }

      if (!type) {
        type = "default";
      } else {
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
          ].map((c) => c.toLowerCase());
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
      const size = Number(headers.get("content-length")) || undefined;

      super(body, { contentType, size });

      this.url = url;
      this.statusText = statusText;
      this.status = extraInit.status || status;
      this.headers = headers;
      this.redirected = extraInit.redirected || false;
      this.type = type;
    }

    get ok() {
      return 200 <= this.status && this.status < 300;
    }

    clone() {
      if (this.bodyUsed) {
        throw TypeError(Body.BodyUsedError);
      }

      const iterators = this.headers.entries();
      const headersList = [];
      for (const header of iterators) {
        headersList.push(header);
      }

      let resBody = this._bodySource;

      if (this._bodySource instanceof ReadableStream) {
        const tees = this._bodySource.tee();
        this._stream = this._bodySource = tees[0];
        resBody = tees[1];
      }

      return new Response(resBody, {
        status: this.status,
        statusText: this.statusText,
        headers: new Headers(headersList),
      });
    }

    static redirect(url, status) {
      if (![301, 302, 303, 307, 308].includes(status)) {
        throw new RangeError(
          "The redirection status must be one of 301, 302, 303, 307 and 308.",
        );
      }
      return new Response(null, {
        status,
        statusText: "",
        headers: [["Location", typeof url === "string" ? url : url.toString()]],
      });
    }
  }

  function sendFetchReq(url, method, headers, body, clientRid) {
    let headerArray = [];
    if (headers) {
      headerArray = Array.from(headers.entries());
    }

    const args = {
      method,
      url,
      headers: headerArray,
      clientRid,
    };

    return opFetch(args, body);
  }

  async function fetch(input, init) {
    let url;
    let method = null;
    let headers = null;
    let body;
    let clientRid = null;
    let redirected = false;
    let remRedirectCount = 20; // TODO: use a better way to handle

    if (typeof input === "string" || input instanceof URL) {
      url = typeof input === "string" ? input : input.href;
      if (init != null) {
        method = init.method || null;
        if (init.headers) {
          headers = init.headers instanceof Headers
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
          } else if (init.body instanceof Blob) {
            body = init.body[blobBytesSymbol];
            contentType = init.body.type;
          } else if (init.body instanceof FormData) {
            let boundary;
            if (headers.has("content-type")) {
              const params = getHeaderValueParams("content-type");
              boundary = params.get("boundary");
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

        if (init.client instanceof HttpClient) {
          clientRid = init.client.rid;
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
    let responseInit = {};
    while (remRedirectCount) {
      const fetchResponse = await sendFetchReq(
        url,
        method,
        headers,
        body,
        clientRid,
      );

      if (
        NULL_BODY_STATUS.includes(fetchResponse.status) ||
        REDIRECT_STATUS.includes(fetchResponse.status)
      ) {
        // We won't use body of received response, so close it now
        // otherwise it will be kept in resource table.
        close(fetchResponse.bodyRid);
        responseBody = null;
      } else {
        responseBody = new ReadableStream({
          async pull(controller) {
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
          cancel() {
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
              redirectUrl = new URL(redirectUrl, url).href;
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

  window.__bootstrap.fetch = {
    fetch,
    Response,
    HttpClient,
    createHttpClient,
  };
})(this);
