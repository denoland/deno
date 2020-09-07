// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { Blob } = window.__bootstrap.blob;
  const { ReadableStream, isReadableStreamDisturbed } =
    window.__bootstrap.streams;
  const { Buffer } = window.__bootstrap.buffer;
  const {
    getHeaderValueParams,
    hasHeaderValueOf,
    isTypedArray,
  } = window.__bootstrap.webUtil;
  const { MultipartParser } = window.__bootstrap.multipart;

  function validateBodyType(owner, bodySource) {
    if (isTypedArray(bodySource)) {
      return true;
    } else if (bodySource instanceof ArrayBuffer) {
      return true;
    } else if (typeof bodySource === "string") {
      return true;
    } else if (bodySource instanceof ReadableStream) {
      return true;
    } else if (bodySource instanceof FormData) {
      return true;
    } else if (bodySource instanceof URLSearchParams) {
      return true;
    } else if (!bodySource) {
      return true; // null body is fine
    }
    throw new Error(
      `Bad ${owner.constructor.name} body type: ${bodySource.constructor.name}`,
    );
  }

  async function bufferFromStream(
    stream,
    size,
  ) {
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

  function bodyToArrayBuffer(bodySource) {
    if (isTypedArray(bodySource)) {
      return bodySource.buffer;
    } else if (bodySource instanceof ArrayBuffer) {
      return bodySource;
    } else if (typeof bodySource === "string") {
      const enc = new TextEncoder();
      return enc.encode(bodySource).buffer;
    } else if (bodySource instanceof ReadableStream) {
      throw new Error(
        `Can't convert stream to ArrayBuffer (try bufferFromStream)`,
      );
    } else if (
      bodySource instanceof FormData ||
      bodySource instanceof URLSearchParams
    ) {
      const enc = new TextEncoder();
      return enc.encode(bodySource.toString()).buffer;
    } else if (!bodySource) {
      return null;
    }
    throw new Error(
      `Body type not implemented: ${bodySource.constructor.name}`,
    );
  }

  const BodyUsedError =
    "Failed to execute 'clone' on 'Body': body is already used";

  class Body {
    #contentType = "";
    #size = undefined;

    constructor(_bodySource, meta) {
      validateBodyType(this, _bodySource);
      this._bodySource = _bodySource;
      this.#contentType = meta.contentType;
      this.#size = meta.size;
      this._stream = null;
    }

    get body() {
      if (this._stream) {
        return this._stream;
      }

      if (!this._bodySource) {
        return null;
      } else if (this._bodySource instanceof ReadableStream) {
        this._stream = this._bodySource;
      } else {
        const buf = bodyToArrayBuffer(this._bodySource);
        if (!(buf instanceof ArrayBuffer)) {
          throw new Error(
            `Expected ArrayBuffer from body`,
          );
        }

        this._stream = new ReadableStream({
          start(controller) {
            controller.enqueue(buf);
            controller.close();
          },
        });
      }

      return this._stream;
    }

    get bodyUsed() {
      if (this.body && isReadableStreamDisturbed(this.body)) {
        return true;
      }
      return false;
    }

    async blob() {
      return new Blob([await this.arrayBuffer()], {
        type: this.#contentType,
      });
    }

    // ref: https://fetch.spec.whatwg.org/#body-mixin
    async formData() {
      const formData = new FormData();
      if (hasHeaderValueOf(this.#contentType, "multipart/form-data")) {
        const params = getHeaderValueParams(this.#contentType);

        // ref: https://tools.ietf.org/html/rfc2046#section-5.1
        const boundary = params.get("boundary");
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
            .forEach((bytes) => {
              if (bytes) {
                const split = bytes.split("=");
                const name = split.shift().replace(/\+/g, " ");
                const value = split.join("=").replace(/\+/g, " ");
                formData.append(
                  decodeURIComponent(name),
                  decodeURIComponent(value),
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

    async text() {
      if (typeof this._bodySource === "string") {
        return this._bodySource;
      }

      const ab = await this.arrayBuffer();
      const decoder = new TextDecoder("utf-8");
      return decoder.decode(ab);
    }

    async json() {
      const raw = await this.text();
      return JSON.parse(raw);
    }

    arrayBuffer() {
      if (this._bodySource instanceof ReadableStream) {
        return bufferFromStream(this._bodySource.getReader(), this.#size);
      }
      return bodyToArrayBuffer(this._bodySource);
    }
  }

  window.__bootstrap.body = {
    Body,
    BodyUsedError,
  };
})(this);
