// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { assert, createResolvable, notImplemented, isTypedArray } from "./util";
import * as flatbuffers from "./flatbuffers";
import { sendAsync } from "./dispatch";
import * as msg from "gen/msg_generated";
import * as domTypes from "./dom_types";
import { TextDecoder, TextEncoder } from "./text_encoding";
import { DenoBlob, bytesSymbol as blobBytesSymbol } from "./blob";
import { Headers } from "./headers";
import * as io from "./io";
import { read, close } from "./files";
import { Buffer } from "./buffer";
import { FormData } from "./form_data";
import { URLSearchParams } from "./url_search_params";

function getHeaderValueParams(value: string): Map<string, string> {
  const params = new Map();
  // Forced to do so for some Map constructor param mismatch
  value
    .split(";")
    .slice(1)
    .map(s => s.trim().split("="))
    .filter(arr => arr.length > 1)
    .map(([k, v]) => [k, v.replace(/^"([^"]*)"$/, "$1")])
    .forEach(([k, v]) => params.set(k, v));
  return params;
}

function hasHeaderValueOf(s: string, value: string) {
  return new RegExp(`^${value}[\t\s]*;?`).test(s);
}

class Body implements domTypes.Body, domTypes.ReadableStream, io.ReadCloser {
  bodyUsed = false;
  private _bodyPromise: null | Promise<ArrayBuffer> = null;
  private _data: ArrayBuffer | null = null;
  readonly locked: boolean = false; // TODO
  readonly body: null | Body = this;

  constructor(private rid: number, readonly contentType: string) {}

  private async _bodyBuffer(): Promise<ArrayBuffer> {
    assert(this._bodyPromise == null);
    const buf = new Buffer();
    try {
      const nread = await buf.readFrom(this);
      const ui8 = buf.bytes();
      assert(ui8.byteLength === nread);
      this._data = ui8.buffer.slice(
        ui8.byteOffset,
        ui8.byteOffset + nread
      ) as ArrayBuffer;
      assert(this._data.byteLength === nread);
    } finally {
      this.close();
    }

    return this._data;
  }

  async arrayBuffer(): Promise<ArrayBuffer> {
    // If we've already bufferred the response, just return it.
    if (this._data != null) {
      return this._data;
    }

    // If there is no _bodyPromise yet, start it.
    if (this._bodyPromise == null) {
      this._bodyPromise = this._bodyBuffer();
    }

    return this._bodyPromise;
  }

  async blob(): Promise<domTypes.Blob> {
    const arrayBuffer = await this.arrayBuffer();
    return new DenoBlob([arrayBuffer], {
      type: this.contentType
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
          .map(s => s.replace(/^[\s\r\n\t]+/, ""));
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
            type: partContentType
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
          .forEach(bytes => {
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

  // tslint:disable-next-line:no-any
  async json(): Promise<any> {
    const text = await this.text();
    return JSON.parse(text);
  }

  async text(): Promise<string> {
    const ab = await this.arrayBuffer();
    const decoder = new TextDecoder("utf-8");
    return decoder.decode(ab);
  }

  read(p: Uint8Array): Promise<io.ReadResult> {
    return read(this.rid, p);
  }

  close(): void {
    close(this.rid);
  }

  async cancel(): Promise<void> {
    return notImplemented();
  }

  getReader(): domTypes.ReadableStreamReader {
    return notImplemented();
  }
}

class Response implements domTypes.Response {
  readonly url: string = "";
  statusText = "FIXME"; // TODO
  readonly type = "basic"; // TODO
  redirected = false; // TODO
  headers: domTypes.Headers;
  readonly trailer: Promise<domTypes.Headers>;
  bodyUsed = false;
  readonly body: Body;

  constructor(
    readonly status: number,
    headersList: Array<[string, string]>,
    rid: number,
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
  }

  async arrayBuffer(): Promise<ArrayBuffer> {
    return this.body.arrayBuffer();
  }

  async blob(): Promise<domTypes.Blob> {
    return this.body.blob();
  }

  async formData(): Promise<domTypes.FormData> {
    return this.body.formData();
  }

  // tslint:disable-next-line:no-any
  async json(): Promise<any> {
    return this.body.json();
  }

  async text(): Promise<string> {
    return this.body.text();
  }

  get ok(): boolean {
    return 200 <= this.status && this.status < 300;
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

    return new Response(this.status, headersList, -1, this.body);
  }
}

function msgHttpRequest(
  builder: flatbuffers.Builder,
  url: string,
  method: null | string,
  headers: null | domTypes.Headers
): flatbuffers.Offset {
  const methodOffset = !method ? -1 : builder.createString(method);
  let fieldsOffset: flatbuffers.Offset = -1;
  const urlOffset = builder.createString(url);
  if (headers) {
    const kvOffsets: flatbuffers.Offset[] = [];
    for (const [key, val] of headers.entries()) {
      const keyOffset = builder.createString(key);
      const valOffset = builder.createString(val);
      msg.KeyValue.startKeyValue(builder);
      msg.KeyValue.addKey(builder, keyOffset);
      msg.KeyValue.addValue(builder, valOffset);
      kvOffsets.push(msg.KeyValue.endKeyValue(builder));
    }
    fieldsOffset = msg.HttpHeader.createFieldsVector(builder, kvOffsets);
  } else {
  }
  msg.HttpHeader.startHttpHeader(builder);
  msg.HttpHeader.addIsRequest(builder, true);
  msg.HttpHeader.addUrl(builder, urlOffset);
  if (methodOffset >= 0) {
    msg.HttpHeader.addMethod(builder, methodOffset);
  }
  if (fieldsOffset >= 0) {
    msg.HttpHeader.addFields(builder, fieldsOffset);
  }
  return msg.HttpHeader.endHttpHeader(builder);
}

/** Fetch a resource from the network. */
export async function fetch(
  input: domTypes.Request | string,
  init?: domTypes.RequestInit
): Promise<Response> {
  let url: string;
  let method: string | null = null;
  let headers: domTypes.Headers | null = null;
  let body: ArrayBufferView | undefined;

  if (typeof input === "string") {
    url = input;
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
        } else {
          // TODO: FormData, ReadableStream
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
  }

  // Send Fetch message
  const builder = flatbuffers.createBuilder();
  const headerOff = msgHttpRequest(builder, url, method, headers);
  msg.Fetch.startFetch(builder);
  msg.Fetch.addHeader(builder, headerOff);
  const resBase = await sendAsync(
    builder,
    msg.Any.Fetch,
    msg.Fetch.endFetch(builder),
    body
  );

  // Decode FetchRes
  assert(msg.Any.FetchRes === resBase.innerType());
  const inner = new msg.FetchRes();
  assert(resBase.inner(inner) != null);

  const header = inner.header()!;
  const bodyRid = inner.bodyRid();
  assert(!header.isRequest());
  const status = header.status();

  const headersList = deserializeHeaderFields(header);

  const response = new Response(status, headersList, bodyRid);
  return response;
}

function deserializeHeaderFields(m: msg.HttpHeader): Array<[string, string]> {
  const out: Array<[string, string]> = [];
  for (let i = 0; i < m.fieldsLength(); i++) {
    const item = m.fields(i)!;
    out.push([item.key()!, item.value()!]);
  }
  return out;
}
