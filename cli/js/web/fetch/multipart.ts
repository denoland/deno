// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { Buffer } from "../../buffer.ts";
import { bytesSymbol } from "../blob.ts";
import { DomFileImpl } from "../dom_file.ts";
import { DenoBlob } from "../blob.ts";
import { TextEncoder, TextDecoder } from "../text_encoding.ts";
import { getHeaderValueParams } from "../util.ts";

const decoder = new TextDecoder();
const encoder = new TextEncoder();
const CR = "\r".charCodeAt(0);
const LF = "\n".charCodeAt(0);

interface MultipartHeaders {
  headers: Headers;
  disposition: Map<string, string>;
}

export class MultipartBuilder {
  readonly boundary: string;
  readonly formData: FormData;
  readonly writer: Buffer;
  constructor(formData: FormData, boundary?: string) {
    this.boundary = boundary ?? this.#createBoundary();
    this.formData = formData;
    this.writer = new Buffer();
  }

  getContentType(): string {
    return `multipart/form-data; boundary=${this.boundary}`;
  }

  getBody(): Uint8Array {
    for (const [fieldName, fieldValue] of this.formData.entries()) {
      if (fieldValue instanceof DomFileImpl) {
        this.#writeFile(fieldName, fieldValue);
      } else this.#writeField(fieldName, fieldValue as string);
    }

    this.writer.writeSync(encoder.encode(`\r\n--${this.boundary}--`));

    return this.writer.bytes();
  }

  #createBoundary = (): string => {
    return (
      "----------" +
      Array.from(Array(32))
        .map(() => Math.random().toString(36)[2] || 0)
        .join("")
    );
  };

  #writeHeaders = (headers: string[][]): void => {
    let buf = this.writer.empty() ? "" : "\r\n";

    buf += `--${this.boundary}\r\n`;
    for (const [key, value] of headers) {
      buf += `${key}: ${value}\r\n`;
    }
    buf += `\r\n`;

    this.writer.write(encoder.encode(buf));
  };

  #writeFileHeaders = (
    field: string,
    filename: string,
    type?: string
  ): void => {
    const headers = [
      [
        "Content-Disposition",
        `form-data; name="${field}"; filename="${filename}"`,
      ],
      ["Content-Type", type || "application/octet-stream"],
    ];
    return this.#writeHeaders(headers);
  };

  #writeFieldHeaders = (field: string): void => {
    const headers = [["Content-Disposition", `form-data; name="${field}"`]];
    return this.#writeHeaders(headers);
  };

  #writeField = (field: string, value: string): void => {
    this.#writeFieldHeaders(field);
    this.writer.writeSync(encoder.encode(value));
  };

  #writeFile = (field: string, value: DomFileImpl): void => {
    this.#writeFileHeaders(field, value.name, value.type);
    this.writer.writeSync(value[bytesSymbol]);
  };
}

export class MultipartParser {
  readonly boundary: string;
  readonly boundaryChars: Uint8Array;
  readonly body: Uint8Array;
  constructor(body: Uint8Array, boundary: string) {
    if (!boundary) {
      throw new TypeError("multipart/form-data must provide a boundary");
    }

    this.boundary = `--${boundary}`;
    this.body = body;
    this.boundaryChars = encoder.encode(this.boundary);
  }

  #parseHeaders = (headersText: string): MultipartHeaders => {
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

    return {
      headers,
      disposition: getHeaderValueParams(
        headers.get("Content-Disposition") ?? ""
      ),
    };
  };

  parse(): FormData {
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
        const headersDone = this.body[i + 1] === CR && this.body[i + 2] === LF;

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
          const content = this.body.subarray(fileStart, i - boundaryIndex - 1);
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
            const blob = new DenoBlob([content], {
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
