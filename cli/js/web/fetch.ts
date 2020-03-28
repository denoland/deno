// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, createResolvable, notImplemented } from "../util.ts";
import { isTypedArray } from "./util.ts";
import * as domTypes from "./dom_types.ts";
import { TextDecoder, TextEncoder } from "./text_encoding.ts";
import { DenoBlob, bytesSymbol as blobBytesSymbol } from "./blob.ts";
import { Headers } from "./headers.ts";
import * as io from "../io.ts";
import { read } from "../ops/io.ts";
import { close } from "../ops/resources.ts";
import { Buffer } from "../buffer.ts";
import { FormData } from "./form_data.ts";
import { URL } from "./url.ts";
import { URLSearchParams } from "./url_search_params.ts";
import { fetch as opFetch, FetchResponse } from "../ops/fetch.ts";
import { DomFileImpl } from "./dom_file.ts";

function getHeaderValueParams(value: string): Map<string, string> {
  const params = new Map();
  // Forced to do so for some Map constructor param mismatch
  value
    .split(";")
    .slice(1)
    .map((s): string[] => s.trim().split("="))
    .filter((arr): boolean => arr.length > 1)
    .map(([k, v]): [string, string] => [k, v.replace(/^"([^"]*)"$/, "$1")])
    .forEach(([k, v]): Map<string, string> => params.set(k, v));
  return params;
}

function hasHeaderValueOf(s: string, value: string): boolean {
  return new RegExp(`^${value}[\t\s]*;?`).test(s);
}

class Body
  implements domTypes.Body, domTypes.ReadableStream<Uint8Array>, io.ReadCloser {
  #bodyUsed = false;
  #bodyPromise: Promise<ArrayBuffer> | null = null;
  #data: ArrayBuffer | null = null;
  #rid: number;
  readonly locked: boolean = false; // TODO
  readonly body: domTypes.ReadableStream<Uint8Array>;

  constructor(rid: number, readonly contentType: string) {
    this.#rid = rid;
    this.body = this;
  }

  #bodyBuffer = async (): Promise<ArrayBuffer> => {
    assert(this.#bodyPromise == null);
    const buf = new Buffer();
    try {
      const nread = await buf.readFrom(this);
      const ui8 = buf.bytes();
      assert(ui8.byteLength === nread);
      this.#data = ui8.buffer.slice(
        ui8.byteOffset,
        ui8.byteOffset + nread
      ) as ArrayBuffer;
      assert(this.#data.byteLength === nread);
    } finally {
      this.close();
    }

    return this.#data;
  };

  // eslint-disable-next-line require-await
  async arrayBuffer(): Promise<ArrayBuffer> {
    // If we've already bufferred the response, just return it.
    if (this.#data != null) {
      return this.#data;
    }

    // If there is no _bodyPromise yet, start it.
    if (this.#bodyPromise == null) {
      this.#bodyPromise = this.#bodyBuffer();
    }

    return this.#bodyPromise;
  }

  async blob(): Promise<domTypes.Blob> {
    const arrayBuffer = await this.arrayBuffer();
    return new DenoBlob([arrayBuffer], {
      type: this.contentType,
    });
  }

  // ref: https://fetch.spec.whatwg.org/#body-mixin
  async formData(): Promise<domTypes.FormData> {
    const formData = new FormData();
    const enc = new TextEncoder();
    if (hasHeaderValueOf(this.contentType, "multipart/form-data")) {
      const params = getHeaderValueParams(this.contentType);
      if (!params.has("boundary")) {
        // TypeError is required by spec
        throw new TypeError("multipart/form-data must provide a boundary");
      }
      // ref: https://tools.ietf.org/html/rfc2046#section-5.1
      const boundary = params.get("boundary")!;
      const dashBoundary = `--${boundary}`;
      const delimiter = `\r\n${dashBoundary}`;
      const closeDelimiter = `${delimiter}--`;

      const body = await this.text();
      let bodyParts: string[];
      const bodyEpilogueSplit = body.split(closeDelimiter);
      if (bodyEpilogueSplit.length < 2) {
        bodyParts = [];
      } else {
        // discard epilogue
        const bodyEpilogueTrimmed = bodyEpilogueSplit[0];
        // first boundary treated special due to optional prefixed \r\n
        const firstBoundaryIndex = bodyEpilogueTrimmed.indexOf(dashBoundary);
        if (firstBoundaryIndex < 0) {
          throw new TypeError("Invalid boundary");
        }
        const bodyPreambleTrimmed = bodyEpilogueTrimmed
          .slice(firstBoundaryIndex + dashBoundary.length)
          .replace(/^[\s\r\n\t]+/, ""); // remove transport-padding CRLF
        // trimStart might not be available
        // Be careful! body-part allows trailing \r\n!
        // (as long as it is not part of `delimiter`)
        bodyParts = bodyPreambleTrimmed
          .split(delimiter)
          .map((s): string => s.replace(/^[\s\r\n\t]+/, ""));
        // TODO: LWSP definition is actually trickier,
        // but should be fine in our case since without headers
        // we should just discard the part
      }
      for (const bodyPart of bodyParts) {
        const headers = new Headers();
        const headerOctetSeperatorIndex = bodyPart.indexOf("\r\n\r\n");
        if (headerOctetSeperatorIndex < 0) {
          continue; // Skip unknown part
        }
        const headerText = bodyPart.slice(0, headerOctetSeperatorIndex);
        const octets = bodyPart.slice(headerOctetSeperatorIndex + 4);

        // TODO: use textproto.readMIMEHeader from deno_std
        const rawHeaders = headerText.split("\r\n");
        for (const rawHeader of rawHeaders) {
          const sepIndex = rawHeader.indexOf(":");
          if (sepIndex < 0) {
            continue; // Skip this header
          }
          const key = rawHeader.slice(0, sepIndex);
          const value = rawHeader.slice(sepIndex + 1);
          headers.set(key, value);
        }
        if (!headers.has("content-disposition")) {
          continue; // Skip unknown part
        }
        // Content-Transfer-Encoding Deprecated
        const contentDisposition = headers.get("content-disposition")!;
        const partContentType = headers.get("content-type") || "text/plain";
        // TODO: custom charset encoding (needs TextEncoder support)
        // const contentTypeCharset =
        //   getHeaderValueParams(partContentType).get("charset") || "";
        if (!hasHeaderValueOf(contentDisposition, "form-data")) {
          continue; // Skip, might not be form-data
        }
        const dispositionParams = getHeaderValueParams(contentDisposition);
        if (!dispositionParams.has("name")) {
          continue; // Skip, unknown name
        }
        const dispositionName = dispositionParams.get("name")!;
        if (dispositionParams.has("filename")) {
          const filename = dispositionParams.get("filename")!;
          const blob = new DenoBlob([enc.encode(octets)], {
            type: partContentType,
          });
          // TODO: based on spec
          // https://xhr.spec.whatwg.org/#dom-formdata-append
          // https://xhr.spec.whatwg.org/#create-an-entry
          // Currently it does not mention how I could pass content-type
          // to the internally created file object...
          formData.append(dispositionName, blob, filename);
        } else {
          formData.append(dispositionName, octets);
        }
      }
      return formData;
    } else if (
      hasHeaderValueOf(this.contentType, "application/x-www-form-urlencoded")
    ) {
      // From https://github.com/github/fetch/blob/master/fetch.js
      // Copyright (c) 2014-2016 GitHub, Inc. MIT License
      const body = await this.text();
      try {
        body
          .trim()
          .split("&")
          .forEach((bytes): void => {
            if (bytes) {
              const split = bytes.split("=");
              const name = split.shift()!.replace(/\+/g, " ");
              const value = split.join("=").replace(/\+/g, " ");
              formData.append(
                decodeURIComponent(name),
                decodeURIComponent(value)
              );
            }
          });
      } catch (e) {
        throw new TypeError("Invalid form urlencoded format");
      }
      return formData;
    } else {
      throw new TypeError("Invalid form data");
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  async json(): Promise<any> {
    const text = await this.text();
    return JSON.parse(text);
  }

  async text(): Promise<string> {
    const ab = await this.arrayBuffer();
    const decoder = new TextDecoder("utf-8");
    return decoder.decode(ab);
  }

  read(p: Uint8Array): Promise<number | io.EOF> {
    this.#bodyUsed = true;
    return read(this.#rid, p);
  }

  close(): Promise<void> {
    close(this.#rid);
    return Promise.resolve();
  }

  cancel(): Promise<void> {
    return notImplemented();
  }

  getReader(options: { mode: "byob" }): domTypes.ReadableStreamBYOBReader;
  getReader(): domTypes.ReadableStreamDefaultReader<Uint8Array>;
  getReader():
    | domTypes.ReadableStreamBYOBReader
    | domTypes.ReadableStreamDefaultReader<Uint8Array> {
    return notImplemented();
  }

  tee(): [domTypes.ReadableStream, domTypes.ReadableStream] {
    return notImplemented();
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array> {
    return io.toAsyncIterator(this);
  }

  get bodyUsed(): boolean {
    return this.#bodyUsed;
  }

  pipeThrough<T>(
    _: {
      writable: domTypes.WritableStream<Uint8Array>;
      readable: domTypes.ReadableStream<T>;
    },
    _options?: domTypes.PipeOptions
  ): domTypes.ReadableStream<T> {
    return notImplemented();
  }

  pipeTo(
    _dest: domTypes.WritableStream<Uint8Array>,
    _options?: domTypes.PipeOptions
  ): Promise<void> {
    return notImplemented();
  }
}

export class Response implements domTypes.Response {
  readonly type: domTypes.ResponseType;
  readonly redirected: boolean;
  headers: domTypes.Headers;
  readonly trailer: Promise<domTypes.Headers>;
  readonly body: Body | null;

  constructor(
    readonly url: string,
    readonly status: number,
    readonly statusText: string,
    headersList: Array<[string, string]>,
    rid: number,
    redirected_: boolean,
    readonly type_: null | domTypes.ResponseType = "default",
    body_: null | Body = null
  ) {
    this.trailer = createResolvable();
    this.headers = new Headers(headersList);
    const contentType = this.headers.get("content-type") || "";

    if (body_ == null) {
      this.body = new Body(rid, contentType);
    } else {
      this.body = body_;
    }

    if (type_ == null) {
      this.type = "default";
    } else {
      this.type = type_;
      if (type_ == "error") {
        // spec: https://fetch.spec.whatwg.org/#concept-network-error
        this.status = 0;
        this.statusText = "";
        this.headers = new Headers();
        this.body = null;
        /* spec for other Response types:
           https://fetch.spec.whatwg.org/#concept-filtered-response-basic
           Please note that type "basic" is not the same thing as "default".*/
      } else if (type_ == "basic") {
        for (const h of this.headers) {
          /* Forbidden Response-Header Names:
             https://fetch.spec.whatwg.org/#forbidden-response-header-name */
          if (["set-cookie", "set-cookie2"].includes(h[0].toLowerCase())) {
            this.headers.delete(h[0]);
          }
        }
      } else if (type_ == "cors") {
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
        for (const h of this.headers) {
          /* Technically this is still not standards compliant because we are
             supposed to allow headers allowed in the
             'Access-Control-Expose-Headers' header in the 'internal response'
             However, this implementation of response doesn't seem to have an
             easy way to access the internal response, so we ignore that
             header.
             TODO(serverhiccups): change how internal responses are handled
             so we can do this properly. */
          if (!allowedHeaders.includes(h[0].toLowerCase())) {
            this.headers.delete(h[0]);
          }
        }
        /* TODO(serverhiccups): Once I fix the 'internal response' thing,
           these actually need to treat the internal response differently */
      } else if (type_ == "opaque" || type_ == "opaqueredirect") {
        this.url = "";
        this.status = 0;
        this.statusText = "";
        this.headers = new Headers();
        this.body = null;
      }
    }

    this.redirected = redirected_;
  }

  #bodyViewable = (): boolean => {
    if (
      this.type == "error" ||
      this.type == "opaque" ||
      this.type == "opaqueredirect" ||
      this.body == undefined
    ) {
      return true;
    }
    return false;
  };

  arrayBuffer(): Promise<ArrayBuffer> {
    /* You have to do the null check here and not in the function because
     * otherwise TS complains about this.body potentially being null */
    if (this.#bodyViewable() || this.body == null) {
      return Promise.reject(new Error("Response body is null"));
    }
    return this.body.arrayBuffer();
  }

  blob(): Promise<domTypes.Blob> {
    if (this.#bodyViewable() || this.body == null) {
      return Promise.reject(new Error("Response body is null"));
    }
    return this.body.blob();
  }

  formData(): Promise<domTypes.FormData> {
    if (this.#bodyViewable() || this.body == null) {
      return Promise.reject(new Error("Response body is null"));
    }
    return this.body.formData();
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  json(): Promise<any> {
    if (this.#bodyViewable() || this.body == null) {
      return Promise.reject(new Error("Response body is null"));
    }
    return this.body.json();
  }

  text(): Promise<string> {
    if (this.#bodyViewable() || this.body == null) {
      return Promise.reject(new Error("Response body is null"));
    }
    return this.body.text();
  }

  get ok(): boolean {
    return 200 <= this.status && this.status < 300;
  }

  get bodyUsed(): boolean {
    if (this.body === null) return false;
    return this.body.bodyUsed;
  }

  clone(): domTypes.Response {
    if (this.bodyUsed) {
      throw new TypeError(
        "Failed to execute 'clone' on 'Response': Response body is already used"
      );
    }

    const iterators = this.headers.entries();
    const headersList: Array<[string, string]> = [];
    for (const header of iterators) {
      headersList.push(header);
    }

    return new Response(
      this.url,
      this.status,
      this.statusText,
      headersList,
      -1,
      this.redirected,
      this.type,
      this.body
    );
  }

  redirect(url: URL | string, status: number): domTypes.Response {
    if (![301, 302, 303, 307, 308].includes(status)) {
      throw new RangeError(
        "The redirection status must be one of 301, 302, 303, 307 and 308."
      );
    }
    return new Response(
      "",
      status,
      "",
      [["Location", typeof url === "string" ? url : url.toString()]],
      -1,
      false,
      "default",
      null
    );
  }
}

function sendFetchReq(
  url: string,
  method: string | null,
  headers: domTypes.Headers | null,
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
  input: domTypes.Request | URL | string,
  init?: domTypes.RequestInit
): Promise<Response> {
  let url: string;
  let method: string | null = null;
  let headers: domTypes.Headers | null = null;
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
        } else if (init.body instanceof URLSearchParams) {
          body = new TextEncoder().encode(init.body.toString());
          contentType = "application/x-www-form-urlencoded;charset=UTF-8";
        } else if (init.body instanceof DenoBlob) {
          body = init.body[blobBytesSymbol];
          contentType = init.body.type;
        } else if (init.body instanceof FormData) {
          let boundary = "";
          if (headers.has("content-type")) {
            const params = getHeaderValueParams("content-type");
            if (params.has("boundary")) {
              boundary = params.get("boundary")!;
            }
          }
          if (!boundary) {
            boundary =
              "----------" +
              Array.from(Array(32))
                .map(() => Math.random().toString(36)[2] || 0)
                .join("");
          }

          let payload = "";
          for (const [fieldName, fieldValue] of init.body.entries()) {
            let part = `\r\n--${boundary}\r\n`;
            part += `Content-Disposition: form-data; name=\"${fieldName}\"`;
            if (fieldValue instanceof DomFileImpl) {
              part += `; filename=\"${fieldValue.name}\"`;
            }
            part += "\r\n";
            if (fieldValue instanceof DomFileImpl) {
              part += `Content-Type: ${
                fieldValue.type || "application/octet-stream"
              }\r\n`;
            }
            part += "\r\n";
            if (fieldValue instanceof DomFileImpl) {
              part += new TextDecoder().decode(fieldValue[blobBytesSymbol]);
            } else {
              part += fieldValue;
            }
            payload += part;
          }
          payload += `\r\n--${boundary}--`;
          body = new TextEncoder().encode(payload);
          contentType = "multipart/form-data; boundary=" + boundary;
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

    //@ts-ignore
    if (input._bodySource) {
      body = new DataView(await input.arrayBuffer());
    }
  }

  while (remRedirectCount) {
    const fetchResponse = await sendFetchReq(url, method, headers, body);

    const response = new Response(
      url,
      fetchResponse.status,
      fetchResponse.statusText,
      fetchResponse.headers,
      fetchResponse.bodyRid,
      redirected
    );
    if ([301, 302, 303, 307, 308].includes(response.status)) {
      // We won't use body of received response, so close it now
      // otherwise it will be kept in resource table.
      close(fetchResponse.bodyRid);
      // We're in a redirect status
      switch ((init && init.redirect) || "follow") {
        case "error":
          /* I suspect that deno will probably crash if you try to use that
             rid, which suggests to me that Response needs to be refactored */
          return new Response("", 0, "", [], -1, false, "error", null);
        case "manual":
          return new Response("", 0, "", [], -1, false, "opaqueredirect", null);
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
  // Return a network error due to too many redirections
  throw notImplemented();
}
