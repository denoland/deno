import * as blob from "./blob.ts";
import * as encoding from "./text_encoding.ts";
import * as domTypes from "./dom_types.d.ts";
import { ReadableStreamImpl } from "./streams/readable_stream.ts";
import { isReadableStreamDisturbed } from "./streams/internals.ts";
import { getHeaderValueParams, hasHeaderValueOf } from "./util.ts";
import { MultipartParser } from "./fetch/multipart.ts";

// only namespace imports work for now, plucking out what we need
const { TextEncoder, TextDecoder } = encoding;
const DenoBlob = blob.DenoBlob;

function validateBodyType(owner: Body, bodySource: BodyInit | null): boolean {
  if (
    bodySource instanceof Int8Array ||
    bodySource instanceof Int16Array ||
    bodySource instanceof Int32Array ||
    bodySource instanceof Uint8Array ||
    bodySource instanceof Uint16Array ||
    bodySource instanceof Uint32Array ||
    bodySource instanceof Uint8ClampedArray ||
    bodySource instanceof Float32Array ||
    bodySource instanceof Float64Array
  ) {
    return true;
  } else if (bodySource instanceof ArrayBuffer) {
    return true;
  } else if (typeof bodySource === "string") {
    return true;
  } else if (bodySource instanceof ReadableStreamImpl) {
    return true;
  } else if (bodySource instanceof FormData) {
    return true;
  } else if (!bodySource) {
    return true; // null body is fine
  }
  throw new Error(
    `Bad ${owner.constructor.name} body type: ${bodySource.constructor.name}`
  );
}

function concatenate(...arrays: Uint8Array[]): ArrayBuffer {
  let totalLength = 0;
  for (const arr of arrays) {
    totalLength += arr.length;
  }
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const arr of arrays) {
    result.set(arr, offset);
    offset += arr.length;
  }
  return result.buffer as ArrayBuffer;
}

async function bufferFromStream(
  stream: ReadableStreamReader
): Promise<ArrayBuffer> {
  const parts: Uint8Array[] = [];
  const encoder = new TextEncoder();

  while (true) {
    const { done, value } = await stream.read();

    if (done) break;

    if (typeof value === "string") {
      parts.push(encoder.encode(value));
    } else if (value instanceof ArrayBuffer) {
      parts.push(new Uint8Array(value));
    } else if (value instanceof Uint8Array) {
      parts.push(value);
    } else if (!value) {
      // noop for undefined
    } else {
      throw new Error("unhandled type on stream read");
    }
  }

  return concatenate(...parts);
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
    if (this.body && isReadableStreamDisturbed(this.body)) {
      return true;
    }
    return false;
  }

  public async blob(): Promise<Blob> {
    return new DenoBlob([await this.arrayBuffer()], {
      type: this.contentType,
    });
  }

  // ref: https://fetch.spec.whatwg.org/#body-mixin
  public async formData(): Promise<FormData> {
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
    if (
      this._bodySource instanceof Int8Array ||
      this._bodySource instanceof Int16Array ||
      this._bodySource instanceof Int32Array ||
      this._bodySource instanceof Uint8Array ||
      this._bodySource instanceof Uint16Array ||
      this._bodySource instanceof Uint32Array ||
      this._bodySource instanceof Uint8ClampedArray ||
      this._bodySource instanceof Float32Array ||
      this._bodySource instanceof Float64Array
    ) {
      return Promise.resolve(this._bodySource.buffer as ArrayBuffer);
    } else if (this._bodySource instanceof ArrayBuffer) {
      return Promise.resolve(this._bodySource);
    } else if (typeof this._bodySource === "string") {
      const enc = new TextEncoder();
      return Promise.resolve(
        enc.encode(this._bodySource).buffer as ArrayBuffer
      );
    } else if (this._bodySource instanceof ReadableStreamImpl) {
      return bufferFromStream(this._bodySource.getReader());
    } else if (this._bodySource instanceof FormData) {
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
