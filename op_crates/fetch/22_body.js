// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../url/internal.d.ts" />
/// <reference path="../url/lib.deno_url.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../file/internal.d.ts" />
/// <reference path="../file/lib.deno_file.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./11_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = globalThis.__bootstrap.webidl;
  const { parseUrlEncoded } = globalThis.__bootstrap.url;
  const { parseFormData, formDataFromEntries, encodeFormData } =
    globalThis.__bootstrap.formData;
  const mimesniff = globalThis.__bootstrap.mimesniff;
  const { isReadableStreamDisturbed } = globalThis.__bootstrap.streams;

  class InnerBody {
    /** @type {ReadableStream<Uint8Array>} */
    stream;
    /** @type {null | Uint8Array | Blob | FormData} */
    source = null;
    /** @type {null | number} */
    length = null;

    /**
     * @param {ReadableStream<Uint8Array>} stream
     */
    constructor(stream) {
      this.stream = stream ?? new ReadableStream();
    }

    /**
     * https://fetch.spec.whatwg.org/#body-unusable
     * @returns {boolean}
     */
    unusable() {
      return this.stream.locked || isReadableStreamDisturbed(this.stream);
    }

    /**
     * https://fetch.spec.whatwg.org/#concept-body-consume-body
     * @returns {Promise<Uint8Array>}
     */
    async consume() {
      if (this.unusable()) throw new TypeError("Body already consumed.");
      const reader = this.stream.getReader();
      /** @type {Uint8Array[]} */
      const chunks = [];
      let totalLength = 0;
      while (true) {
        const { value: chunk, done } = await reader.read();
        if (done) break;
        chunks.push(chunk);
        totalLength += chunk.byteLength;
      }
      const finalBuffer = new Uint8Array(totalLength);
      let i = 0;
      for (const chunk of chunks) {
        finalBuffer.set(chunk, i);
        i += chunk.byteLength;
      }
      return finalBuffer;
    }

    /**
     * @returns {InnerBody}
     */
    clone() {
      const [out1, out2] = this.stream.tee();
      this.stream = out1;
      const second = new InnerBody(out2);
      second.source = core.deserialize(core.serialize(this.source));
      second.length = this.length;
      return second;
    }
  }

  /**
   * @param {any} prototype 
   * @param {symbol} bodySymbol 
   * @param {symbol} mimeTypeSymbol 
   * @returns {void}
   */
  function mixinBody(prototype, bodySymbol, mimeTypeSymbol) {
    function consumeBody(object) {
      if (object[bodySymbol] !== null) {
        return object[bodySymbol].consume();
      }
      return Promise.resolve(new Uint8Array());
    }

    /** @type {PropertyDescriptorMap} */
    const mixin = {
      body: {
        /**
         * @returns {ReadableStream<Uint8Array> | null}
         */
        get() {
          webidl.assertBranded(this, prototype);
          if (this[bodySymbol] === null) {
            return null;
          } else {
            return this[bodySymbol].stream;
          }
        },
      },
      bodyUsed: {
        /**
         * @returns {boolean}
         */
        get() {
          webidl.assertBranded(this, prototype);
          if (this[bodySymbol] !== null) {
            return isReadableStreamDisturbed(this[bodySymbol].stream);
          }
          return false;
        },
      },
      arrayBuffer: {
        /** @returns {Promise<ArrayBuffer>} */
        value: async function arrayBuffer() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "ArrayBuffer");
        },
      },
      blob: {
        /** @returns {Promise<Blob>} */
        value: async function blob() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "Blob", this[mimeTypeSymbol]);
        },
      },
      formData: {
        /** @returns {Promise<FormData>} */
        value: async function formData() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "FormData", this[mimeTypeSymbol]);
        },
      },
      json: {
        /** @returns {Promise<any>} */
        value: async function json() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "JSON");
        },
      },
      text: {
        /** @returns {Promise<string>} */
        value: async function text() {
          webidl.assertBranded(this, prototype);
          const body = await consumeBody(this);
          return packageData(body, "text");
        },
      },
    };
    return Object.defineProperties(prototype.prototype, mixin);
  }

  const decoder = new TextDecoder();

  /**
   * https://fetch.spec.whatwg.org/#concept-body-package-data
   * @param {Uint8Array} bytes
   * @param {"ArrayBuffer" | "Blob" | "FormData" | "JSON" | "text"} type
   * @param {MimeType | null} [mimeType]
   */
  function packageData(bytes, type, mimeType) {
    switch (type) {
      case "ArrayBuffer":
        return bytes.buffer;
      case "Blob":
        return new Blob([bytes], {
          type: mimeType !== null ? mimesniff.serializeMimeType(mimeType) : "",
        });
      case "FormData": {
        if (mimeType !== null) {
          if (mimeType !== null) {
            const essence = mimesniff.essence(mimeType);
            if (essence === "multipart/form-data") {
              const boundary = mimeType.parameters.get("boundary");
              if (boundary === null) {
                throw new TypeError(
                  "Missing boundary parameter in mime type of multipart formdata.",
                );
              }
              return parseFormData(bytes, boundary);
            } else if (essence === "application/x-www-form-urlencoded") {
              const entries = parseUrlEncoded(bytes);
              return formDataFromEntries(
                entries.map((x) => ({ name: x[0], value: x[1] })),
              );
            }
          }
          throw new TypeError("Invalid form data");
        }
        throw new TypeError("Missing content type");
      }
      case "JSON":
        return JSON.parse(decoder.decode(bytes));
      case "text":
        return decoder.decode(bytes);
    }
  }

  const encoder = new TextEncoder();

  /**
   * @param {BodyInit} object
   * @returns {{body: InnerBody, contentType: string | null}}
   */
  function extractBody(object) {
    let stream;
    let source = null;
    let length = null;
    let contentType = null;
    if (object instanceof Blob) {
      stream = object.stream();
      source = object;
      length = object.size;
      if (object.type.length !== 0) {
        contentType = object.type;
      }
    } else if (ArrayBuffer.isView(object) || object instanceof ArrayBuffer) {
      const u8 = ArrayBuffer.isView(object)
        ? new Uint8Array(
          object.buffer,
          object.byteOffset,
          object.byteLength,
        )
        : new Uint8Array(object);
      const copy = u8.slice(0, u8.byteLength);
      source = copy;
    } else if (object instanceof FormData) {
      const res = encodeFormData(object);
      stream = new ReadableStream({
        start(controller) {
          controller.enqueue(res.body);
          controller.close();
        },
      });
      source = object;
      length = res.body.byteLength;
      contentType = res.contentType;
    } else if (object instanceof URLSearchParams) {
      source = encoder.encode(object.toString());
      contentType = "application/x-www-form-urlencoded;charset=UTF-8";
    } else if (typeof object === "string") {
      source = encoder.encode(object);
      contentType = "text/plain;charset=UTF-8";
    } else if (object instanceof ReadableStream) {
      stream = object;
      if (object.locked || isReadableStreamDisturbed(object)) {
        throw new TypeError("ReadableStream is locked or disturbed");
      }
    }
    if (source instanceof Uint8Array) {
      stream = new ReadableStream({
        start(controller) {
          controller.enqueue(source);
          controller.close();
        },
      });
      length = source.byteLength;
    }
    const body = new InnerBody(stream);
    body.source = source;
    body.length = length;
    return { body, contentType };
  }

  webidl.converters["BodyInit"] = (V, opts) => {
    // Union for (ReadableStream or Blob or ArrayBufferView or ArrayBuffer or FormData or URLSearchParams or USVString)
    if (V instanceof ReadableStream) {
      // TODO(lucacasonato): ReadableStream is not branded
      return V;
    } else if (V instanceof Blob) {
      return webidl.converters["Blob"](V, opts);
    } else if (V instanceof FormData) {
      return webidl.converters["FormData"](V, opts);
    } else if (V instanceof URLSearchParams) {
      // TODO(lucacasonato): URLSearchParams is not branded
      return V;
    }
    if (typeof V === "object") {
      if (V instanceof ArrayBuffer || V instanceof SharedArrayBuffer) {
        return webidl.converters["ArrayBuffer"](V, opts);
      }
      if (ArrayBuffer.isView(V)) {
        return webidl.converters["ArrayBufferView"](V, opts);
      }
    }
    return webidl.converters["USVString"](V, opts);
  };
  webidl.converters["BodyInit?"] = webidl.createNullableConverter(
    webidl.converters["BodyInit"],
  );

  window.__bootstrap.fetchBody = { mixinBody, InnerBody, extractBody };
})(globalThis);
