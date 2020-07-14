// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as blob from "./blob.ts";
import * as encoding from "./text_encoding.ts";
import type * as domTypes from "./dom_types.d.ts";
import { ReadableStreamImpl } from "./streams/readable_stream.ts";
import { isReadableStreamDisturbed } from "./streams/internals.ts";
import { Buffer } from "../buffer.ts";

import {
  getHeaderValueParams,
  hasHeaderValueOf,
  isTypedArray,
} from "./util.ts";
import { MultipartParser } from "./fetch/multipart.ts";

// only namespace imports work for now, plucking out what we need
const { TextEncoder, TextDecoder } = encoding;
const DenoBlob = blob.DenoBlob;

interface BodyMeta {
  contentType: string;
  size?: number;
}

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
  stream: ReadableStreamReader,
  size?: number
): Promise<ArrayBuffer> {
  const encoder = new TextEncoder();
  const buffer = new Buffer();

  if (size) {
    // grow to avoid unnecessary allocations & copies
    buffer.grow(size);
  }

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
  #contentType: string;
  #size: number | undefined;
  constructor(protected _bodySource: BodyInit | null, meta: BodyMeta) {
    validateBodyType(this, _bodySource);
    this._bodySource = _bodySource;
    this.#contentType = meta.contentType;
    this.#size = meta.size;
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
    if (this.body && isReadableStreamDisturbed(this.body)) {
      return true;
    }
    return false;
  }

  public async blob(): Promise<Blob> {
    return new DenoBlob([await this.arrayBuffer()], {
      type: this.#contentType,
    });
  }

  // ref: https://fetch.spec.whatwg.org/#body-mixin
  public async formData(): Promise<FormData> {
    const formData = new FormData();
    if (hasHeaderValueOf(this.#contentType, "multipart/form-data")) {
      const params = getHeaderValueParams(this.#contentType);

      // ref: https://tools.ietf.org/html/rfc2046#section-5.1
      const boundary = params.get("boundary")!;
      const body = new Uint8Array(await this.arrayBuffer());
      const multipartParser = new MultipartParser(body, boundary);

      return multipartParser.parse();
    } else if (
      hasHeaderValueOf(this.#contentType, "application/x-www-form-urlencoded")
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

  public async text(): Promise<string> {
    if (typeof this._bodySource === "string") {
      return this._bodySource;
    }

    const ab = await this.arrayBuffer();
    const decoder = new TextDecoder("utf-8");
    return decoder.decode(ab);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  public async json(): Promise<any> {
    const raw = await this.text();
    return JSON.parse(raw);
  }

  public arrayBuffer(): Promise<ArrayBuffer> {
    if (isTypedArray(this._bodySource)) {
      return Promise.resolve(this._bodySource.buffer as ArrayBuffer);
    } else if (this._bodySource instanceof ArrayBuffer) {
      return Promise.resolve(this._bodySource);
    } else if (typeof this._bodySource === "string") {
      const enc = new TextEncoder();
      return Promise.resolve(
        enc.encode(this._bodySource).buffer as ArrayBuffer
      );
    } else if (this._bodySource instanceof ReadableStreamImpl) {
      return bufferFromStream(this._bodySource.getReader(), this.#size);
    } else if (
      this._bodySource instanceof FormData ||
      this._bodySource instanceof URLSearchParams
    ) {
      const enc = new TextEncoder();
      return Promise.resolve(
        enc.encode(this._bodySource.toString()).buffer as ArrayBuffer
      );
    } else if (!this._bodySource) {
      return Promise.resolve(new ArrayBuffer(0));
    }
    throw new Error(
      `Body type not yet implemented: ${this._bodySource.constructor.name}`
    );
  }
}
