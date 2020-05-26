import { Buffer } from "../../buffer.ts";
import { bytesSymbol } from "../blob.ts";
import { DomFileImpl } from "../dom_file.ts";
import { TextEncoder } from "../text_encoding.ts";

const encoder = new TextEncoder();

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
