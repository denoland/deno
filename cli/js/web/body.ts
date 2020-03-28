import * as formData from "./form_data.ts";
import * as blob from "./blob.ts";
import * as encoding from "./text_encoding.ts";
import * as headers from "./headers.ts";
import * as domTypes from "./dom_types.ts";
import { ReadableStream } from "./streams/mod.ts";

const { Headers } = headers;

// only namespace imports work for now, plucking out what we need
const { FormData } = formData;
const { TextEncoder, TextDecoder } = encoding;
const Blob = blob.DenoBlob;
const DenoBlob = blob.DenoBlob;

type ReadableStreamReader = domTypes.ReadableStreamReader;

interface ReadableStreamController {
  enqueue(chunk: string | ArrayBuffer): void;
  close(): void;
}

export type BodySource =
  | domTypes.Blob
  | domTypes.BufferSource
  | domTypes.FormData
  | domTypes.URLSearchParams
  | domTypes.ReadableStream
  | string;

function validateBodyType(owner: Body, bodySource: BodySource): boolean {
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
  } else if (bodySource instanceof ReadableStream) {
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

function bufferFromStream(stream: ReadableStreamReader): Promise<ArrayBuffer> {
  return new Promise((resolve, reject): void => {
    const parts: Uint8Array[] = [];
    const encoder = new TextEncoder();
    // recurse
    (function pump(): void {
      stream
        .read()
        .then(({ done, value }): void => {
          if (done) {
            return resolve(concatenate(...parts));
          }

          if (typeof value === "string") {
            parts.push(encoder.encode(value));
          } else if (value instanceof ArrayBuffer) {
            parts.push(new Uint8Array(value));
          } else if (!value) {
            // noop for undefined
          } else {
            reject("unhandled type on stream read");
          }

          return pump();
        })
        .catch((err): void => {
          reject(err);
        });
    })();
  });
}

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

export const BodyUsedError =
  "Failed to execute 'clone' on 'Body': body is already used";

export class Body implements domTypes.Body {
  protected _stream: domTypes.ReadableStream<string | ArrayBuffer> | null;

  constructor(protected _bodySource: BodySource, readonly contentType: string) {
    validateBodyType(this, _bodySource);
    this._bodySource = _bodySource;
    this.contentType = contentType;
    this._stream = null;
  }

  get body(): domTypes.ReadableStream | null {
    if (this._stream) {
      return this._stream;
    }

    if (this._bodySource instanceof ReadableStream) {
      // @ts-ignore
      this._stream = this._bodySource;
    }
    if (typeof this._bodySource === "string") {
      const bodySource = this._bodySource;
      this._stream = new ReadableStream({
        start(controller: ReadableStreamController): void {
          controller.enqueue(bodySource);
          controller.close();
        },
      }) as domTypes.ReadableStream<ArrayBuffer | string>;
    }
    return this._stream;
  }

  get bodyUsed(): boolean {
    if (this.body && this.body.locked) {
      return true;
    }
    return false;
  }

  public async blob(): Promise<domTypes.Blob> {
    return new Blob([await this.arrayBuffer()]);
  }

  // ref: https://fetch.spec.whatwg.org/#body-mixin
  public async formData(): Promise<domTypes.FormData> {
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
    } else if (this._bodySource instanceof ReadableStream) {
      // @ts-ignore
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
