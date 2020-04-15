System.register(
  "$deno$/web/body.ts",
  [
    "$deno$/web/blob.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/web/streams/mod.ts",
  ],
  function (exports_100, context_100) {
    "use strict";
    let blob, encoding, mod_ts_2, TextEncoder, TextDecoder, DenoBlob, Body;
    const __moduleName = context_100 && context_100.id;
    function validateBodyType(owner, bodySource) {
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
      } else if (bodySource instanceof mod_ts_2.ReadableStream) {
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
    function concatenate(...arrays) {
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
      return result.buffer;
    }
    function bufferFromStream(stream) {
      return new Promise((resolve, reject) => {
        const parts = [];
        const encoder = new TextEncoder();
        // recurse
        (function pump() {
          stream
            .read()
            .then(({ done, value }) => {
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
            .catch((err) => {
              reject(err);
            });
        })();
      });
    }
    function getHeaderValueParams(value) {
      const params = new Map();
      // Forced to do so for some Map constructor param mismatch
      value
        .split(";")
        .slice(1)
        .map((s) => s.trim().split("="))
        .filter((arr) => arr.length > 1)
        .map(([k, v]) => [k, v.replace(/^"([^"]*)"$/, "$1")])
        .forEach(([k, v]) => params.set(k, v));
      return params;
    }
    function hasHeaderValueOf(s, value) {
      return new RegExp(`^${value}[\t\s]*;?`).test(s);
    }
    return {
      setters: [
        function (blob_3) {
          blob = blob_3;
        },
        function (encoding_1) {
          encoding = encoding_1;
        },
        function (mod_ts_2_1) {
          mod_ts_2 = mod_ts_2_1;
        },
      ],
      execute: function () {
        // only namespace imports work for now, plucking out what we need
        (TextEncoder = encoding.TextEncoder),
          (TextDecoder = encoding.TextDecoder);
        DenoBlob = blob.DenoBlob;
        exports_100(
          "BodyUsedError",
          "Failed to execute 'clone' on 'Body': body is already used"
        );
        Body = class Body {
          constructor(_bodySource, contentType) {
            this._bodySource = _bodySource;
            this.contentType = contentType;
            validateBodyType(this, _bodySource);
            this._bodySource = _bodySource;
            this.contentType = contentType;
            this._stream = null;
          }
          get body() {
            if (this._stream) {
              return this._stream;
            }
            if (this._bodySource instanceof mod_ts_2.ReadableStream) {
              // @ts-ignore
              this._stream = this._bodySource;
            }
            if (typeof this._bodySource === "string") {
              const bodySource = this._bodySource;
              this._stream = new mod_ts_2.ReadableStream({
                start(controller) {
                  controller.enqueue(bodySource);
                  controller.close();
                },
              });
            }
            return this._stream;
          }
          get bodyUsed() {
            if (this.body && this.body.locked) {
              return true;
            }
            return false;
          }
          async blob() {
            return new DenoBlob([await this.arrayBuffer()]);
          }
          // ref: https://fetch.spec.whatwg.org/#body-mixin
          async formData() {
            const formData = new FormData();
            const enc = new TextEncoder();
            if (hasHeaderValueOf(this.contentType, "multipart/form-data")) {
              const params = getHeaderValueParams(this.contentType);
              if (!params.has("boundary")) {
                // TypeError is required by spec
                throw new TypeError(
                  "multipart/form-data must provide a boundary"
                );
              }
              // ref: https://tools.ietf.org/html/rfc2046#section-5.1
              const boundary = params.get("boundary");
              const dashBoundary = `--${boundary}`;
              const delimiter = `\r\n${dashBoundary}`;
              const closeDelimiter = `${delimiter}--`;
              const body = await this.text();
              let bodyParts;
              const bodyEpilogueSplit = body.split(closeDelimiter);
              if (bodyEpilogueSplit.length < 2) {
                bodyParts = [];
              } else {
                // discard epilogue
                const bodyEpilogueTrimmed = bodyEpilogueSplit[0];
                // first boundary treated special due to optional prefixed \r\n
                const firstBoundaryIndex = bodyEpilogueTrimmed.indexOf(
                  dashBoundary
                );
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
                  .map((s) => s.replace(/^[\s\r\n\t]+/, ""));
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
                const contentDisposition = headers.get("content-disposition");
                const partContentType =
                  headers.get("content-type") || "text/plain";
                // TODO: custom charset encoding (needs TextEncoder support)
                // const contentTypeCharset =
                //   getHeaderValueParams(partContentType).get("charset") || "";
                if (!hasHeaderValueOf(contentDisposition, "form-data")) {
                  continue; // Skip, might not be form-data
                }
                const dispositionParams = getHeaderValueParams(
                  contentDisposition
                );
                if (!dispositionParams.has("name")) {
                  continue; // Skip, unknown name
                }
                const dispositionName = dispositionParams.get("name");
                if (dispositionParams.has("filename")) {
                  const filename = dispositionParams.get("filename");
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
              hasHeaderValueOf(
                this.contentType,
                "application/x-www-form-urlencoded"
              )
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
          async text() {
            if (typeof this._bodySource === "string") {
              return this._bodySource;
            }
            const ab = await this.arrayBuffer();
            const decoder = new TextDecoder("utf-8");
            return decoder.decode(ab);
          }
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          async json() {
            const raw = await this.text();
            return JSON.parse(raw);
          }
          arrayBuffer() {
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
              return Promise.resolve(this._bodySource.buffer);
            } else if (this._bodySource instanceof ArrayBuffer) {
              return Promise.resolve(this._bodySource);
            } else if (typeof this._bodySource === "string") {
              const enc = new TextEncoder();
              return Promise.resolve(enc.encode(this._bodySource).buffer);
            } else if (this._bodySource instanceof mod_ts_2.ReadableStream) {
              // @ts-ignore
              return bufferFromStream(this._bodySource.getReader());
            } else if (this._bodySource instanceof FormData) {
              const enc = new TextEncoder();
              return Promise.resolve(
                enc.encode(this._bodySource.toString()).buffer
              );
            } else if (!this._bodySource) {
              return Promise.resolve(new ArrayBuffer(0));
            }
            throw new Error(
              `Body type not yet implemented: ${this._bodySource.constructor.name}`
            );
          }
        };
        exports_100("Body", Body);
      },
    };
  }
);
