// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { DenoBlob } from "./blob.ts";
import { TextEncoder, TextDecoder } from "./text_encoding.ts";
import * as domTypes from "./dom_types.d.ts";
import { ReadableStreamImpl } from "./streams/readable_stream.ts";
import { isReadableStreamDisturbed } from "./streams/internals.ts";
import { Buffer } from "../buffer.ts";

import {
  getHeaderValueParams,
  hasHeaderValueOf,
  isTypedArray,
} from "./util.ts";
import { MultipartParser } from "./fetch/multipart.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function validateBodyType(owner: Body, bodySource: BodyInit | null): boolean {
  if (isTypedArray(bodySource)) {
    return true;
  } else if (bodySource instanceof ArrayBuffer) {
    return true;
  } else if (typeof bodySource === "string") {
    return true;
  } else if (bodySource instanceof ReadableStreamImpl) {
    return true;
  } else if (bodySource instanceof FormData) {
    return true;
  } else if (bodySource instanceof URLSearchParams) {
    return true;
  } else if (!bodySource) {
    return true; // null body is fine
  }
  throw new Error(
    `Bad ${owner.constructor.name} body type: ${bodySource.constructor.name}`
  );
}

async function bufferFromStream(
  stream: ReadableStreamReader
): Promise<ArrayBuffer> {
  const buffer = new Buffer();

  while (true) {
    const { done, value } = await stream.read();

    if (done) break;

    if (typeof value === "string") {
      buffer.writeSync(encoder.encode(value));
    } else if (value instanceof ArrayBuffer) {
      buffer.writeSync(new Uint8Array(value));
    } else if (value instanceof Uint8Array) {
      buffer.writeSync(value);
    } else if (!value) {
      // noop for undefined
    } else {
      throw new Error("unhandled type on stream read");
    }
  }

  return buffer.bytes().buffer;
}

export const BodyUsedError =
  "Failed to execute 'clone' on 'Body': body is already used";

export class Body implements domTypes.Body {
  protected _stream: ReadableStreamImpl<string | ArrayBuffer> | null;

  constructor(
    protected _bodySource: BodyInit | null,
    readonly contentType: string
  ) {
    validateBodyType(this, _bodySource);
    this._bodySource = _bodySource;
    this.contentType = contentType;
    this._stream = null;
  }

  get body(): ReadableStream | null {
    if (this._stream) {
      return this._stream;
    }

    if (this._bodySource instanceof ReadableStreamImpl) {
      this._stream = this._bodySource;
    }
    if (typeof this._bodySource === "string") {
      const bodySource = this._bodySource;
      this._stream = new ReadableStreamImpl<string | ArrayBuffer>({
        start(controller: ReadableStreamDefaultController): void {
          controller.enqueue(bodySource);
          controller.close();
        },
      });
    }
    return this._stream;
  }

  get bodyUsed(): boolean {
    return !!(this.body && isReadableStreamDisturbed(this.body));
  }

  async blob(): Promise<Blob> {
    return new DenoBlob([await this.arrayBuffer()], { type: this.contentType });
  }

  // ref: https://fetch.spec.whatwg.org/#body-mixin
  async formData(): Promise<FormData> {
    const formData = new FormData();

    if (hasHeaderValueOf(this.contentType, "multipart/form-data")) {
      const params = getHeaderValueParams(this.contentType);

      // ref: https://tools.ietf.org/html/rfc2046#section-5.1
      const boundary = params.get("boundary")!;
      const body = new Uint8Array(await this.arrayBuffer());
      const multipartParser = new MultipartParser(body, boundary);

      return multipartParser.parse();
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
      } catch {
        throw new TypeError("Invalid form urlencoded format");
      }
      return formData;
    } else {
      throw new TypeError("Invalid form data");
    }
  }

  async text(): Promise<string> {
    if (typeof this._bodySource === "string") {
      return this._bodySource;
    }
    return decoder.decode(await this.arrayBuffer());
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  async json(): Promise<any> {
    return JSON.parse(await this.text());
  }

  arrayBuffer(): Promise<ArrayBuffer> {
    if (isTypedArray(this._bodySource)) {
      return Promise.resolve(this._bodySource.buffer as ArrayBuffer);
    } else if (this._bodySource instanceof ArrayBuffer) {
      return Promise.resolve(this._bodySource);
    } else if (typeof this._bodySource === "string") {
      return Promise.resolve(
        encoder.encode(this._bodySource).buffer as ArrayBuffer
      );
    } else if (this._bodySource instanceof ReadableStreamImpl) {
      return bufferFromStream(this._bodySource.getReader());
    } else if (
      this._bodySource instanceof FormData ||
      this._bodySource instanceof URLSearchParams
    ) {
      return Promise.resolve(
        encoder.encode(this._bodySource.toString()).buffer as ArrayBuffer
      );
    } else if (!this._bodySource) {
      return Promise.resolve(new ArrayBuffer(0));
    }
    throw new Error(
      `Body type not yet implemented: ${this._bodySource.constructor.name}`
    );
  }
}
