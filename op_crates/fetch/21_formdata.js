// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../file/internal.d.ts" />
/// <reference path="../file/lib.deno_file.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./11_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const webidl = globalThis.__bootstrap.webidl;
  const { Blob, File, _byteSequence } = globalThis.__bootstrap.file;

  const entryList = Symbol("entry list");

  /**
   * @param {string} name
   * @param {string | Blob} value
   * @param {string | undefined} filename
   * @returns {FormDataEntry}
   */
  function createEntry(name, value, filename) {
    if (value instanceof Blob && !(value instanceof File)) {
      value = new File([value[_byteSequence]], "blob", { type: value.type });
    }
    if (value instanceof File && filename !== undefined) {
      value = new File([value[_byteSequence]], filename, {
        type: value.type,
        lastModified: value.lastModified,
      });
    }
    return {
      name,
      // @ts-expect-error because TS is not smart enough
      value,
    };
  }

  /**
   * @typedef FormDataEntry
   * @property {string} name
   * @property {FormDataEntryValue} value
   */

  class FormData {
    get [Symbol.toStringTag]() {
      return "FormData";
    }

    /** @type {FormDataEntry[]} */
    [entryList] = [];

    /** @param {void} form */
    constructor(form) {
      if (form !== undefined) {
        webidl.illegalConstructor();
      }
      this[webidl.brand] = webidl.brand;
    }

    /**
     * @param {string} name 
     * @param {string | Blob} valueOrBlobValue 
     * @param {string} [filename]
     * @returns {void}
     */
    append(name, valueOrBlobValue, filename) {
      webidl.assertBranded(this, FormData);
      const prefix = "Failed to execute 'append' on 'FormData'";
      webidl.requiredArguments(arguments.length, 2, { prefix });

      name = webidl.converters["USVString"](name, {
        prefix,
        context: "Argument 1",
      });
      if (valueOrBlobValue instanceof Blob) {
        valueOrBlobValue = webidl.converters["Blob"](valueOrBlobValue, {
          prefix,
          context: "Argument 2",
        });
        if (filename !== undefined) {
          filename = webidl.converters["USVString"](filename, {
            prefix,
            context: "Argument 3",
          });
        }
      } else {
        valueOrBlobValue = webidl.converters["USVString"](valueOrBlobValue, {
          prefix,
          context: "Argument 2",
        });
      }

      const entry = createEntry(name, valueOrBlobValue, filename);

      this[entryList].push(entry);
    }

    /**
     * @param {string} name 
     * @returns {void}
     */
    delete(name) {
      webidl.assertBranded(this, FormData);
      const prefix = "Failed to execute 'name' on 'FormData'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      name = webidl.converters["USVString"](name, {
        prefix,
        context: "Argument 1",
      });

      const list = this[entryList];
      for (let i = 0; i < list.length; i++) {
        if (list[i].name === name) {
          list.splice(i, 1);
          i--;
        }
      }
    }

    /**
     * @param {string} name 
     * @returns {FormDataEntryValue | null}
     */
    get(name) {
      webidl.assertBranded(this, FormData);
      const prefix = "Failed to execute 'get' on 'FormData'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      name = webidl.converters["USVString"](name, {
        prefix,
        context: "Argument 1",
      });

      for (const entry of this[entryList]) {
        if (entry.name === name) return entry.value;
      }
      return null;
    }

    /**
     * @param {string} name 
     * @returns {FormDataEntryValue[]}
     */
    getAll(name) {
      webidl.assertBranded(this, FormData);
      const prefix = "Failed to execute 'getAll' on 'FormData'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      name = webidl.converters["USVString"](name, {
        prefix,
        context: "Argument 1",
      });

      const returnList = [];
      for (const entry of this[entryList]) {
        if (entry.name === name) returnList.push(entry.value);
      }
      return returnList;
    }

    /**
     * @param {string} name 
     * @returns {boolean}
     */
    has(name) {
      webidl.assertBranded(this, FormData);
      const prefix = "Failed to execute 'has' on 'FormData'";
      webidl.requiredArguments(arguments.length, 1, { prefix });

      name = webidl.converters["USVString"](name, {
        prefix,
        context: "Argument 1",
      });

      for (const entry of this[entryList]) {
        if (entry.name === name) return true;
      }
      return false;
    }

    /**
     * @param {string} name 
     * @param {string | Blob} valueOrBlobValue 
     * @param {string} [filename]
     * @returns {void}
     */
    set(name, valueOrBlobValue, filename) {
      webidl.assertBranded(this, FormData);
      const prefix = "Failed to execute 'set' on 'FormData'";
      webidl.requiredArguments(arguments.length, 2, { prefix });

      name = webidl.converters["USVString"](name, {
        prefix,
        context: "Argument 1",
      });
      if (valueOrBlobValue instanceof Blob) {
        valueOrBlobValue = webidl.converters["Blob"](valueOrBlobValue, {
          prefix,
          context: "Argument 2",
        });
        if (filename !== undefined) {
          filename = webidl.converters["USVString"](filename, {
            prefix,
            context: "Argument 3",
          });
        }
      } else {
        valueOrBlobValue = webidl.converters["USVString"](valueOrBlobValue, {
          prefix,
          context: "Argument 2",
        });
      }

      const entry = createEntry(name, valueOrBlobValue, filename);

      const list = this[entryList];
      let added = false;
      for (let i = 0; i < list.length; i++) {
        if (list[i].name === name) {
          if (!added) {
            list[i] = entry;
            added = true;
          } else {
            list.splice(i, 1);
            i--;
          }
        }
      }
      if (!added) {
        list.push(entry);
      }
    }
  }

  webidl.mixinPairIterable("FormData", FormData, entryList, "name", "value");

  const encoder = new TextEncoder();

  class MultipartBuilder {
    /**
     * @param {FormData} formData 
     */
    constructor(formData) {
      this.entryList = formData[entryList];
      this.boundary = this.#createBoundary();
      /** @type {Uint8Array[]} */
      this.chunks = [];
    }

    /** 
     * @returns {string}
     */
    getContentType() {
      return `multipart/form-data; boundary=${this.boundary}`;
    }

    /** 
     * @returns {Uint8Array}
     */
    getBody() {
      for (const { name, value } of this.entryList) {
        if (value instanceof File) {
          this.#writeFile(name, value);
        } else this.#writeField(name, value);
      }

      this.chunks.push(encoder.encode(`\r\n--${this.boundary}--`));

      let totalLength = 0;
      for (const chunk of this.chunks) {
        totalLength += chunk.byteLength;
      }

      const finalBuffer = new Uint8Array(totalLength);
      let i = 0;
      for (const chunk of this.chunks) {
        finalBuffer.set(chunk, i);
        i += chunk.byteLength;
      }

      return finalBuffer;
    }

    #createBoundary = () => {
      return (
        "----------" +
        Array.from(Array(32))
          .map(() => Math.random().toString(36)[2] || 0)
          .join("")
      );
    };

    /** 
     * @param {[string, string][]} headers
     * @returns {void}
     */
    #writeHeaders = (headers) => {
      let buf = (this.chunks.length === 0) ? "" : "\r\n";

      buf += `--${this.boundary}\r\n`;
      for (const [key, value] of headers) {
        buf += `${key}: ${value}\r\n`;
      }
      buf += `\r\n`;

      this.chunks.push(encoder.encode(buf));
    };

    /** 
     * @param {string} field
     * @param {string} filename
     * @param {string} [type]
     * @returns {void}
     */
    #writeFileHeaders = (
      field,
      filename,
      type,
    ) => {
      /** @type {[string, string][]} */
      const headers = [
        [
          "Content-Disposition",
          `form-data; name="${field}"; filename="${filename}"`,
        ],
        ["Content-Type", type || "application/octet-stream"],
      ];
      return this.#writeHeaders(headers);
    };

    /**
     * @param {string} field
     * @returns {void}
     */
    #writeFieldHeaders = (field) => {
      /** @type {[string, string][]} */
      const headers = [["Content-Disposition", `form-data; name="${field}"`]];
      return this.#writeHeaders(headers);
    };

    /**
     * @param {string} field
     * @param {string} value
     * @returns {void}
     */
    #writeField = (field, value) => {
      this.#writeFieldHeaders(field);
      this.chunks.push(encoder.encode(value));
    };

    /**
     * @param {string} field
     * @param {File} value
     * @returns {void}
     */
    #writeFile = (field, value) => {
      this.#writeFileHeaders(field, value.name, value.type);
      this.chunks.push(value[_byteSequence]);
    };
  }

  /**
   * @param {FormData} formdata 
   * @returns {{body: Uint8Array, contentType: string}}
   */
  function encodeFormData(formdata) {
    const builder = new MultipartBuilder(formdata);
    return {
      body: builder.getBody(),
      contentType: builder.getContentType(),
    };
  }

  /**
   * @param {string} value
   * @returns {Map<string, string>}
   */
  function parseContentDisposition(value) {
    /** @type {Map<string, string>} */
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

  const LF = "\n".codePointAt(0);
  const CR = "\r".codePointAt(0);
  const decoder = new TextDecoder("utf-8");

  class MultipartParser {
    /**
     * @param {Uint8Array} body 
     * @param {string | undefined} boundary 
     */
    constructor(body, boundary) {
      if (!boundary) {
        throw new TypeError("multipart/form-data must provide a boundary");
      }

      this.boundary = `--${boundary}`;
      this.body = body;
      this.boundaryChars = encoder.encode(this.boundary);
    }

    /**
     * @param {string} headersText
     * @returns {{ headers: Headers, disposition: Map<string, string> }}
     */
    #parseHeaders = (headersText) => {
      const headers = new Headers();
      const rawHeaders = headersText.split("\r\n");
      for (const rawHeader of rawHeaders) {
        const sepIndex = rawHeader.indexOf(":");
        if (sepIndex < 0) {
          continue; // Skip this header
        }
        const key = rawHeader.slice(0, sepIndex);
        const value = rawHeader.slice(sepIndex + 1);
        headers.set(key, value);
      }

      const disposition = parseContentDisposition(
        headers.get("Content-Disposition") ?? "",
      );

      return { headers, disposition };
    };

    /**
     * @returns {FormData}
     */
    parse() {
      // Body must be at least 2 boundaries + \r\n + -- on the last boundary.
      if (this.body.length < (this.boundary.length * 2) + 4) {
        throw new TypeError("Form data too short to be valid.");
      }

      const formData = new FormData();
      let headerText = "";
      let boundaryIndex = 0;
      let state = 0;
      let fileStart = 0;

      for (let i = 0; i < this.body.length; i++) {
        const byte = this.body[i];
        const prevByte = this.body[i - 1];
        const isNewLine = byte === LF && prevByte === CR;

        if (state === 1 || state === 2 || state == 3) {
          headerText += String.fromCharCode(byte);
        }
        if (state === 0 && isNewLine) {
          state = 1;
        } else if (state === 1 && isNewLine) {
          state = 2;
          const headersDone = this.body[i + 1] === CR &&
            this.body[i + 2] === LF;

          if (headersDone) {
            state = 3;
          }
        } else if (state === 2 && isNewLine) {
          state = 3;
        } else if (state === 3 && isNewLine) {
          state = 4;
          fileStart = i + 1;
        } else if (state === 4) {
          if (this.boundaryChars[boundaryIndex] !== byte) {
            boundaryIndex = 0;
          } else {
            boundaryIndex++;
          }

          if (boundaryIndex >= this.boundary.length) {
            const { headers, disposition } = this.#parseHeaders(headerText);
            const content = this.body.subarray(
              fileStart,
              i - boundaryIndex - 1,
            );
            // https://fetch.spec.whatwg.org/#ref-for-dom-body-formdata
            const filename = disposition.get("filename");
            const name = disposition.get("name");

            state = 5;
            // Reset
            boundaryIndex = 0;
            headerText = "";

            if (!name) {
              continue; // Skip, unknown name
            }

            if (filename) {
              const blob = new Blob([content], {
                type: headers.get("Content-Type") || "application/octet-stream",
              });
              formData.append(name, blob, filename);
            } else {
              formData.append(name, decoder.decode(content));
            }
          }
        } else if (state === 5 && isNewLine) {
          state = 1;
        }
      }

      return formData;
    }
  }

  /**
   * @param {Uint8Array} body 
   * @param {string | undefined} boundary
   * @returns {FormData}
   */
  function parseFormData(body, boundary) {
    const parser = new MultipartParser(body, boundary);
    return parser.parse();
  }

  /**
   * @param {FormDataEntry[]} entries
   * @returns {FormData} 
   */
  function formDataFromEntries(entries) {
    const fd = new FormData();
    fd[entryList] = entries;
    return fd;
  }

  webidl.converters["FormData"] = webidl
    .createInterfaceConverter("FormData", FormData);

  globalThis.__bootstrap.formData = {
    FormData,
    encodeFormData,
    parseFormData,
    formDataFromEntries,
  };
})(globalThis);
