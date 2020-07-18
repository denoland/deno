// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { equal, findIndex, findLastIndex, hasPrefix } from "../bytes/mod.ts";
import { copyN } from "../io/ioutil.ts";
import { MultiReader } from "../io/readers.ts";
import { extname } from "../path/mod.ts";
import { tempFile } from "../io/util.ts";
import { BufReader, BufWriter } from "../io/bufio.ts";
import { encoder } from "../encoding/utf8.ts";
import { assert } from "../_util/assert.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { hasOwnProperty } from "../_util/has_own_property.ts";

/** FormFile object */
export interface FormFile {
  /** filename  */
  filename: string;
  /** content-type header value of file */
  type: string;
  /** byte size of file */
  size: number;
  /** in-memory content of file. Either content or tempfile is set  */
  content?: Uint8Array;
  /** temporal file path.
   * Set if file size is bigger than specified max-memory size at reading form
   * */
  tempfile?: string;
}

/** Type guard for FormFile */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function isFormFile(x: any): x is FormFile {
  return hasOwnProperty(x, "filename") && hasOwnProperty(x, "type");
}

function randomBoundary(): string {
  let boundary = "--------------------------";
  for (let i = 0; i < 24; i++) {
    boundary += Math.floor(Math.random() * 16).toString(16);
  }
  return boundary;
}

/**
 * Checks whether `buf` should be considered to match the boundary.
 *
 * The prefix is "--boundary" or "\r\n--boundary" or "\n--boundary", and the
 * caller has verified already that `hasPrefix(buf, prefix)` is true.
 *
 * `matchAfterPrefix()` returns `1` if the buffer does match the boundary,
 * meaning the prefix is followed by a dash, space, tab, cr, nl, or EOF.
 *
 * It returns `-1` if the buffer definitely does NOT match the boundary,
 * meaning the prefix is followed by some other character.
 * For example, "--foobar" does not match "--foo".
 *
 * It returns `0` more input needs to be read to make the decision,
 * meaning that `buf.length` and `prefix.length` are the same.
 */
export function matchAfterPrefix(
  buf: Uint8Array,
  prefix: Uint8Array,
  eof: boolean,
): -1 | 0 | 1 {
  if (buf.length === prefix.length) {
    return eof ? 1 : 0;
  }
  const c = buf[prefix.length];
  if (
    c === " ".charCodeAt(0) ||
    c === "\t".charCodeAt(0) ||
    c === "\r".charCodeAt(0) ||
    c === "\n".charCodeAt(0) ||
    c === "-".charCodeAt(0)
  ) {
    return 1;
  }
  return -1;
}

/**
 * Scans `buf` to identify how much of it can be safely returned as part of the
 * `PartReader` body.
 *
 * @param buf - The buffer to search for boundaries.
 * @param dashBoundary - Is "--boundary".
 * @param newLineDashBoundary - Is "\r\n--boundary" or "\n--boundary", depending
 * on what mode we are in. The comments below (and the name) assume
 * "\n--boundary", but either is accepted.
 * @param total - The number of bytes read out so far. If total == 0, then a
 * leading "--boundary" is recognized.
 * @param eof - Whether `buf` contains the final bytes in the stream before EOF.
 * If `eof` is false, more bytes are expected to follow.
 * @returns The number of data bytes from buf that can be returned as part of
 * the `PartReader` body.
 */
export function scanUntilBoundary(
  buf: Uint8Array,
  dashBoundary: Uint8Array,
  newLineDashBoundary: Uint8Array,
  total: number,
  eof: boolean,
): number | null {
  if (total === 0) {
    // At beginning of body, allow dashBoundary.
    if (hasPrefix(buf, dashBoundary)) {
      switch (matchAfterPrefix(buf, dashBoundary, eof)) {
        case -1:
          return dashBoundary.length;
        case 0:
          return 0;
        case 1:
          return null;
      }
    }
    if (hasPrefix(dashBoundary, buf)) {
      return 0;
    }
  }

  // Search for "\n--boundary".
  const i = findIndex(buf, newLineDashBoundary);
  if (i >= 0) {
    switch (matchAfterPrefix(buf.slice(i), newLineDashBoundary, eof)) {
      case -1:
        return i + newLineDashBoundary.length;
      case 0:
        return i;
      case 1:
        return i > 0 ? i : null;
    }
  }
  if (hasPrefix(newLineDashBoundary, buf)) {
    return 0;
  }

  // Otherwise, anything up to the final \n is not part of the boundary and so
  // must be part of the body. Also, if the section from the final \n onward is
  // not a prefix of the boundary, it too must be part of the body.
  const j = findLastIndex(buf, newLineDashBoundary.slice(0, 1));
  if (j >= 0 && hasPrefix(newLineDashBoundary, buf.slice(j))) {
    return j;
  }

  return buf.length;
}

class PartReader implements Deno.Reader, Deno.Closer {
  n: number | null = 0;
  total = 0;

  constructor(private mr: MultipartReader, public readonly headers: Headers) {}

  async read(p: Uint8Array): Promise<number | null> {
    const br = this.mr.bufReader;

    // Read into buffer until we identify some data to return,
    // or we find a reason to stop (boundary or EOF).
    let peekLength = 1;
    while (this.n === 0) {
      peekLength = Math.max(peekLength, br.buffered());
      const peekBuf = await br.peek(peekLength);
      if (peekBuf === null) {
        throw new Deno.errors.UnexpectedEof();
      }
      const eof = peekBuf.length < peekLength;
      this.n = scanUntilBoundary(
        peekBuf,
        this.mr.dashBoundary,
        this.mr.newLineDashBoundary,
        this.total,
        eof,
      );
      if (this.n === 0) {
        // Force buffered I/O to read more into buffer.
        assert(eof === false);
        peekLength++;
      }
    }

    if (this.n === null) {
      return null;
    }

    const nread = Math.min(p.length, this.n);
    const buf = p.subarray(0, nread);
    const r = await br.readFull(buf);
    assert(r === buf);
    this.n -= nread;
    this.total += nread;
    return nread;
  }

  close(): void {}

  private contentDisposition!: string;
  private contentDispositionParams!: { [key: string]: string };

  private getContentDispositionParams(): { [key: string]: string } {
    if (this.contentDispositionParams) return this.contentDispositionParams;
    const cd = this.headers.get("content-disposition");
    const params: { [key: string]: string } = {};
    assert(cd != null, "content-disposition must be set");
    const comps = decodeURI(cd).split(";");
    this.contentDisposition = comps[0];
    comps
      .slice(1)
      .map((v: string): string => v.trim())
      .map((kv: string): void => {
        const [k, v] = kv.split("=");
        if (v) {
          const s = v.charAt(0);
          const e = v.charAt(v.length - 1);
          if ((s === e && s === '"') || s === "'") {
            params[k] = v.substr(1, v.length - 2);
          } else {
            params[k] = v;
          }
        }
      });
    return (this.contentDispositionParams = params);
  }

  get fileName(): string {
    return this.getContentDispositionParams()["filename"];
  }

  get formName(): string {
    const p = this.getContentDispositionParams();
    if (this.contentDisposition === "form-data") {
      return p["name"];
    }
    return "";
  }
}

function skipLWSPChar(u: Uint8Array): Uint8Array {
  const ret = new Uint8Array(u.length);
  const sp = " ".charCodeAt(0);
  const ht = "\t".charCodeAt(0);
  let j = 0;
  for (let i = 0; i < u.length; i++) {
    if (u[i] === sp || u[i] === ht) continue;
    ret[j++] = u[i];
  }
  return ret.slice(0, j);
}

export interface MultipartFormData {
  file(key: string): FormFile | FormFile[] | undefined;
  value(key: string): string | undefined;
  entries(): IterableIterator<
    [string, string | FormFile | FormFile[] | undefined]
  >;
  [Symbol.iterator](): IterableIterator<
    [string, string | FormFile | FormFile[] | undefined]
  >;
  /** Remove all tempfiles */
  removeAll(): Promise<void>;
}

/** Reader for parsing multipart/form-data */
export class MultipartReader {
  readonly newLine = encoder.encode("\r\n");
  readonly newLineDashBoundary = encoder.encode(`\r\n--${this.boundary}`);
  readonly dashBoundaryDash = encoder.encode(`--${this.boundary}--`);
  readonly dashBoundary = encoder.encode(`--${this.boundary}`);
  readonly bufReader: BufReader;

  constructor(reader: Deno.Reader, private boundary: string) {
    this.bufReader = new BufReader(reader);
  }

  /** Read all form data from stream.
   * If total size of stored data in memory exceed maxMemory,
   * overflowed file data will be written to temporal files.
   * String field values are never written to files.
   * null value means parsing or writing to file was failed in some reason.
   * @param maxMemory maximum memory size to store file in memory. bytes. @default 10485760 (10MB)
   *  */
  async readForm(maxMemory = 10 << 20): Promise<MultipartFormData> {
    const fileMap = new Map<string, FormFile | FormFile[]>();
    const valueMap = new Map<string, string>();
    let maxValueBytes = maxMemory + (10 << 20);
    const buf = new Deno.Buffer(new Uint8Array(maxValueBytes));
    for (;;) {
      const p = await this.nextPart();
      if (p === null) {
        break;
      }
      if (p.formName === "") {
        continue;
      }
      buf.reset();
      if (!p.fileName) {
        // value
        const n = await copyN(p, buf, maxValueBytes);
        maxValueBytes -= n;
        if (maxValueBytes < 0) {
          throw new RangeError("message too large");
        }
        const value = new TextDecoder().decode(buf.bytes());
        valueMap.set(p.formName, value);
        continue;
      }
      // file
      let formFile: FormFile | FormFile[] | undefined;
      const n = await copyN(p, buf, maxValueBytes);
      const contentType = p.headers.get("content-type");
      assert(contentType != null, "content-type must be set");
      if (n > maxMemory) {
        // too big, write to disk and flush buffer
        const ext = extname(p.fileName);
        const { file, filepath } = await tempFile(".", {
          prefix: "multipart-",
          postfix: ext,
        });
        try {
          const size = await Deno.copy(new MultiReader(buf, p), file);

          file.close();
          formFile = {
            filename: p.fileName,
            type: contentType,
            tempfile: filepath,
            size,
          };
        } catch (e) {
          await Deno.remove(filepath);
          throw e;
        }
      } else {
        formFile = {
          filename: p.fileName,
          type: contentType,
          content: buf.bytes(),
          size: buf.length,
        };
        maxMemory -= n;
        maxValueBytes -= n;
      }
      if (formFile) {
        const mapVal = fileMap.get(p.formName);
        if (mapVal !== undefined) {
          if (Array.isArray(mapVal)) {
            mapVal.push(formFile);
          } else {
            fileMap.set(p.formName, [mapVal, formFile]);
          }
        } else {
          fileMap.set(p.formName, formFile);
        }
      }
    }
    return multipatFormData(fileMap, valueMap);
  }

  private currentPart: PartReader | undefined;
  private partsRead = 0;

  private async nextPart(): Promise<PartReader | null> {
    if (this.currentPart) {
      this.currentPart.close();
    }
    if (equal(this.dashBoundary, encoder.encode("--"))) {
      throw new Error("boundary is empty");
    }
    let expectNewPart = false;
    for (;;) {
      const line = await this.bufReader.readSlice("\n".charCodeAt(0));
      if (line === null) {
        throw new Deno.errors.UnexpectedEof();
      }
      if (this.isBoundaryDelimiterLine(line)) {
        this.partsRead++;
        const r = new TextProtoReader(this.bufReader);
        const headers = await r.readMIMEHeader();
        if (headers === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        const np = new PartReader(this, headers);
        this.currentPart = np;
        return np;
      }
      if (this.isFinalBoundary(line)) {
        return null;
      }
      if (expectNewPart) {
        throw new Error(`expecting a new Part; got line ${line}`);
      }
      if (this.partsRead === 0) {
        continue;
      }
      if (equal(line, this.newLine)) {
        expectNewPart = true;
        continue;
      }
      throw new Error(`unexpected line in nextPart(): ${line}`);
    }
  }

  private isFinalBoundary(line: Uint8Array): boolean {
    if (!hasPrefix(line, this.dashBoundaryDash)) {
      return false;
    }
    const rest = line.slice(this.dashBoundaryDash.length, line.length);
    return rest.length === 0 || equal(skipLWSPChar(rest), this.newLine);
  }

  private isBoundaryDelimiterLine(line: Uint8Array): boolean {
    if (!hasPrefix(line, this.dashBoundary)) {
      return false;
    }
    const rest = line.slice(this.dashBoundary.length);
    return equal(skipLWSPChar(rest), this.newLine);
  }
}

function multipatFormData(
  fileMap: Map<string, FormFile | FormFile[]>,
  valueMap: Map<string, string>,
): MultipartFormData {
  function file(key: string): FormFile | FormFile[] | undefined {
    return fileMap.get(key);
  }
  function value(key: string): string | undefined {
    return valueMap.get(key);
  }
  function* entries(): IterableIterator<
    [string, string | FormFile | FormFile[] | undefined]
  > {
    yield* fileMap;
    yield* valueMap;
  }
  async function removeAll(): Promise<void> {
    const promises: Array<Promise<void>> = [];
    for (const val of fileMap.values()) {
      if (Array.isArray(val)) {
        for (const subVal of val) {
          if (!subVal.tempfile) continue;
          promises.push(Deno.remove(subVal.tempfile));
        }
      } else {
        if (!val.tempfile) continue;
        promises.push(Deno.remove(val.tempfile));
      }
    }
    await Promise.all(promises);
  }
  return {
    file,
    value,
    entries,
    removeAll,
    [Symbol.iterator](): IterableIterator<
      [string, string | FormFile | FormFile[] | undefined]
    > {
      return entries();
    },
  };
}

class PartWriter implements Deno.Writer {
  closed = false;
  private readonly partHeader: string;
  private headersWritten = false;

  constructor(
    private writer: Deno.Writer,
    readonly boundary: string,
    public headers: Headers,
    isFirstBoundary: boolean,
  ) {
    let buf = "";
    if (isFirstBoundary) {
      buf += `--${boundary}\r\n`;
    } else {
      buf += `\r\n--${boundary}\r\n`;
    }
    for (const [key, value] of headers.entries()) {
      buf += `${key}: ${value}\r\n`;
    }
    buf += `\r\n`;
    this.partHeader = buf;
  }

  close(): void {
    this.closed = true;
  }

  async write(p: Uint8Array): Promise<number> {
    if (this.closed) {
      throw new Error("part is closed");
    }
    if (!this.headersWritten) {
      await this.writer.write(encoder.encode(this.partHeader));
      this.headersWritten = true;
    }
    return this.writer.write(p);
  }
}

function checkBoundary(b: string): string {
  if (b.length < 1 || b.length > 70) {
    throw new Error(`invalid boundary length: ${b.length}`);
  }
  const end = b.length - 1;
  for (let i = 0; i < end; i++) {
    const c = b.charAt(i);
    if (!c.match(/[a-zA-Z0-9'()+_,\-./:=?]/) || (c === " " && i !== end)) {
      throw new Error("invalid boundary character: " + c);
    }
  }
  return b;
}

/** Writer for creating multipart/form-data */
export class MultipartWriter {
  private readonly _boundary: string;

  get boundary(): string {
    return this._boundary;
  }

  private lastPart: PartWriter | undefined;
  private bufWriter: BufWriter;
  private isClosed = false;

  constructor(private readonly writer: Deno.Writer, boundary?: string) {
    if (boundary !== void 0) {
      this._boundary = checkBoundary(boundary);
    } else {
      this._boundary = randomBoundary();
    }
    this.bufWriter = new BufWriter(writer);
  }

  formDataContentType(): string {
    return `multipart/form-data; boundary=${this.boundary}`;
  }

  private createPart(headers: Headers): Deno.Writer {
    if (this.isClosed) {
      throw new Error("multipart: writer is closed");
    }
    if (this.lastPart) {
      this.lastPart.close();
    }
    const part = new PartWriter(
      this.writer,
      this.boundary,
      headers,
      !this.lastPart,
    );
    this.lastPart = part;
    return part;
  }

  createFormFile(field: string, filename: string): Deno.Writer {
    const h = new Headers();
    h.set(
      "Content-Disposition",
      `form-data; name="${field}"; filename="${filename}"`,
    );
    h.set("Content-Type", "application/octet-stream");
    return this.createPart(h);
  }

  createFormField(field: string): Deno.Writer {
    const h = new Headers();
    h.set("Content-Disposition", `form-data; name="${field}"`);
    h.set("Content-Type", "application/octet-stream");
    return this.createPart(h);
  }

  async writeField(field: string, value: string): Promise<void> {
    const f = await this.createFormField(field);
    await f.write(encoder.encode(value));
  }

  async writeFile(
    field: string,
    filename: string,
    file: Deno.Reader,
  ): Promise<void> {
    const f = await this.createFormFile(field, filename);
    await Deno.copy(file, f);
  }

  private flush(): Promise<void> {
    return this.bufWriter.flush();
  }

  /** Close writer. No additional data can be written to stream */
  async close(): Promise<void> {
    if (this.isClosed) {
      throw new Error("multipart: writer is closed");
    }
    if (this.lastPart) {
      this.lastPart.close();
      this.lastPart = void 0;
    }
    await this.writer.write(encoder.encode(`\r\n--${this.boundary}--\r\n`));
    await this.flush();
    this.isClosed = true;
  }
}
