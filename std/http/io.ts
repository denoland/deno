import { BufReader, BufWriter } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { assert } from "../testing/asserts.ts";
import { encoder } from "../strings/mod.ts";
import { ServerRequest, Response } from "./server.ts";
import { STATUS_TEXT } from "./http_status.ts";

export function emptyReader(): Deno.Reader {
  return {
    read(_: Uint8Array): Promise<number | Deno.EOF> {
      return Promise.resolve(Deno.EOF);
    },
  };
}

export function bodyReader(contentLength: number, r: BufReader): Deno.Reader {
  let totalRead = 0;
  let finished = false;
  async function read(buf: Uint8Array): Promise<number | Deno.EOF> {
    if (finished) return Deno.EOF;
    let result: number | Deno.EOF;
    const remaining = contentLength - totalRead;
    if (remaining >= buf.byteLength) {
      result = await r.read(buf);
    } else {
      const readBuf = buf.subarray(0, remaining);
      result = await r.read(readBuf);
    }
    if (result !== Deno.EOF) {
      totalRead += result;
    }
    finished = totalRead === contentLength;
    return result;
  }
  return { read };
}

export function chunkedBodyReader(h: Headers, r: BufReader): Deno.Reader {
  // Based on https://tools.ietf.org/html/rfc2616#section-19.4.6
  const tp = new TextProtoReader(r);
  let finished = false;
  const chunks: Array<{
    offset: number;
    data: Uint8Array;
  }> = [];
  async function read(buf: Uint8Array): Promise<number | Deno.EOF> {
    if (finished) return Deno.EOF;
    const [chunk] = chunks;
    if (chunk) {
      const chunkRemaining = chunk.data.byteLength - chunk.offset;
      const readLength = Math.min(chunkRemaining, buf.byteLength);
      for (let i = 0; i < readLength; i++) {
        buf[i] = chunk.data[chunk.offset + i];
      }
      chunk.offset += readLength;
      if (chunk.offset === chunk.data.byteLength) {
        chunks.shift();
        // Consume \r\n;
        if ((await tp.readLine()) === Deno.EOF) {
          throw new Deno.errors.UnexpectedEof();
        }
      }
      return readLength;
    }
    const line = await tp.readLine();
    if (line === Deno.EOF) throw new Deno.errors.UnexpectedEof();
    // TODO: handle chunk extension
    const [chunkSizeString] = line.split(";");
    const chunkSize = parseInt(chunkSizeString, 16);
    if (Number.isNaN(chunkSize) || chunkSize < 0) {
      throw new Error("Invalid chunk size");
    }
    if (chunkSize > 0) {
      if (chunkSize > buf.byteLength) {
        let eof = await r.readFull(buf);
        if (eof === Deno.EOF) {
          throw new Deno.errors.UnexpectedEof();
        }
        const restChunk = new Uint8Array(chunkSize - buf.byteLength);
        eof = await r.readFull(restChunk);
        if (eof === Deno.EOF) {
          throw new Deno.errors.UnexpectedEof();
        } else {
          chunks.push({
            offset: 0,
            data: restChunk,
          });
        }
        return buf.byteLength;
      } else {
        const bufToFill = buf.subarray(0, chunkSize);
        const eof = await r.readFull(bufToFill);
        if (eof === Deno.EOF) {
          throw new Deno.errors.UnexpectedEof();
        }
        // Consume \r\n
        if ((await tp.readLine()) === Deno.EOF) {
          throw new Deno.errors.UnexpectedEof();
        }
        return chunkSize;
      }
    } else {
      assert(chunkSize === 0);
      // Consume \r\n
      if ((await r.readLine()) === Deno.EOF) {
        throw new Deno.errors.UnexpectedEof();
      }
      await readTrailers(h, r);
      finished = true;
      return Deno.EOF;
    }
  }
  return { read };
}

const kProhibitedTrailerHeaders = [
  "transfer-encoding",
  "content-length",
  "trailer",
];

/**
 * Read trailer headers from reader and append values to headers.
 * "trailer" field will be deleted.
 * */
export async function readTrailers(
  headers: Headers,
  r: BufReader
): Promise<void> {
  const keys = parseTrailer(headers.get("trailer"));
  if (!keys) return;
  const tp = new TextProtoReader(r);
  const result = await tp.readMIMEHeader();
  assert(result != Deno.EOF, "trailer must be set");
  for (const [k, v] of result) {
    if (!keys.has(k)) {
      throw new Error("Undeclared trailer field");
    }
    keys.delete(k);
    headers.append(k, v);
  }
  assert(keys.size === 0, "Missing trailers");
  headers.delete("trailer");
}

function parseTrailer(field: string | null): Set<string> | undefined {
  if (field == null) {
    return undefined;
  }
  const keys = field.split(",").map((v) => v.trim());
  if (keys.length === 0) {
    throw new Error("Empty trailer");
  }
  for (const invalid of kProhibitedTrailerHeaders) {
    if (keys.includes(invalid)) {
      throw new Error(`Prohibited field for trailer`);
    }
  }
  return new Set(keys);
}

export async function writeChunkedBody(
  w: Deno.Writer,
  r: Deno.Reader
): Promise<void> {
  const writer = BufWriter.create(w);
  for await (const chunk of Deno.toAsyncIterator(r)) {
    if (chunk.byteLength <= 0) continue;
    const start = encoder.encode(`${chunk.byteLength.toString(16)}\r\n`);
    const end = encoder.encode("\r\n");
    await writer.write(start);
    await writer.write(chunk);
    await writer.write(end);
  }

  const endChunk = encoder.encode("0\r\n\r\n");
  await writer.write(endChunk);
}

/** write trailer headers to writer. it mostly should be called after writeResponse */
export async function writeTrailers(
  w: Deno.Writer,
  headers: Headers,
  trailers: Headers
): Promise<void> {
  const trailer = headers.get("trailer");
  if (trailer === null) {
    throw new Error('response headers must have "trailer" header field');
  }
  const transferEncoding = headers.get("transfer-encoding");
  if (transferEncoding === null || !transferEncoding.match(/^chunked/)) {
    throw new Error(
      `trailer headers is only allowed for "transfer-encoding: chunked": got "${transferEncoding}"`
    );
  }
  const writer = BufWriter.create(w);
  const trailerHeaderFields = trailer
    .split(",")
    .map((s) => s.trim().toLowerCase());
  for (const f of trailerHeaderFields) {
    assert(
      !kProhibitedTrailerHeaders.includes(f),
      `"${f}" is prohibited for trailer header`
    );
  }
  for (const [key, value] of trailers) {
    assert(
      trailerHeaderFields.includes(key),
      `Not trailer header field: ${key}`
    );
    await writer.write(encoder.encode(`${key}: ${value}\r\n`));
  }
  await writer.write(encoder.encode("\r\n"));
  await writer.flush();
}

export function setContentLength(r: Response): void {
  if (!r.headers) {
    r.headers = new Headers();
  }

  if (r.body) {
    if (!r.headers.has("content-length")) {
      // typeof r.body === "string" handled in writeResponse.
      if (r.body instanceof Uint8Array) {
        const bodyLength = r.body.byteLength;
        r.headers.set("content-length", bodyLength.toString());
      } else {
        r.headers.set("transfer-encoding", "chunked");
      }
    }
  }
}

export async function writeResponse(
  w: Deno.Writer,
  r: Response
): Promise<void> {
  const protoMajor = 1;
  const protoMinor = 1;
  const statusCode = r.status || 200;
  const statusText = STATUS_TEXT.get(statusCode);
  const writer = BufWriter.create(w);
  if (!statusText) {
    throw new Deno.errors.InvalidData("Bad status code");
  }
  if (!r.body) {
    r.body = new Uint8Array();
  }
  if (typeof r.body === "string") {
    r.body = encoder.encode(r.body);
  }

  let out = `HTTP/${protoMajor}.${protoMinor} ${statusCode} ${statusText}\r\n`;

  setContentLength(r);
  assert(r.headers != null);
  const headers = r.headers;

  for (const [key, value] of headers) {
    out += `${key}: ${value}\r\n`;
  }
  out += "\r\n";

  const header = encoder.encode(out);
  const n = await writer.write(header);
  assert(n === header.byteLength);

  if (r.body instanceof Uint8Array) {
    const n = await writer.write(r.body);
    assert(n === r.body.byteLength);
  } else if (headers.has("content-length")) {
    const contentLength = headers.get("content-length");
    assert(contentLength != null);
    const bodyLength = parseInt(contentLength);
    const n = await Deno.copy(writer, r.body);
    assert(n === bodyLength);
  } else {
    await writeChunkedBody(writer, r.body);
  }
  if (r.trailers) {
    const t = await r.trailers();
    await writeTrailers(writer, headers, t);
  }
  await writer.flush();
}

/**
 * ParseHTTPVersion parses a HTTP version string.
 * "HTTP/1.0" returns (1, 0, true).
 * Ported from https://github.com/golang/go/blob/f5c43b9/src/net/http/request.go#L766-L792
 */
export function parseHTTPVersion(vers: string): [number, number] {
  switch (vers) {
    case "HTTP/1.1":
      return [1, 1];

    case "HTTP/1.0":
      return [1, 0];

    default: {
      const Big = 1000000; // arbitrary upper bound
      const digitReg = /^\d+$/; // test if string is only digit

      if (!vers.startsWith("HTTP/")) {
        break;
      }

      const dot = vers.indexOf(".");
      if (dot < 0) {
        break;
      }

      const majorStr = vers.substring(vers.indexOf("/") + 1, dot);
      const major = parseInt(majorStr);
      if (
        !digitReg.test(majorStr) ||
        isNaN(major) ||
        major < 0 ||
        major > Big
      ) {
        break;
      }

      const minorStr = vers.substring(dot + 1);
      const minor = parseInt(minorStr);
      if (
        !digitReg.test(minorStr) ||
        isNaN(minor) ||
        minor < 0 ||
        minor > Big
      ) {
        break;
      }

      return [major, minor];
    }
  }

  throw new Error(`malformed HTTP version ${vers}`);
}

export async function readRequest(
  conn: Deno.Conn,
  bufr: BufReader
): Promise<ServerRequest | Deno.EOF> {
  const tp = new TextProtoReader(bufr);
  const firstLine = await tp.readLine(); // e.g. GET /index.html HTTP/1.0
  if (firstLine === Deno.EOF) return Deno.EOF;
  const headers = await tp.readMIMEHeader();
  if (headers === Deno.EOF) throw new Deno.errors.UnexpectedEof();

  const req = new ServerRequest();
  req.conn = conn;
  req.r = bufr;
  [req.method, req.url, req.proto] = firstLine.split(" ", 3);
  [req.protoMinor, req.protoMajor] = parseHTTPVersion(req.proto);
  req.headers = headers;
  fixLength(req);
  return req;
}

function fixLength(req: ServerRequest): void {
  const contentLength = req.headers.get("Content-Length");
  if (contentLength) {
    const arrClen = contentLength.split(",");
    if (arrClen.length > 1) {
      const distinct = [...new Set(arrClen.map((e): string => e.trim()))];
      if (distinct.length > 1) {
        throw Error("cannot contain multiple Content-Length headers");
      } else {
        req.headers.set("Content-Length", distinct[0]);
      }
    }
    const c = req.headers.get("Content-Length");
    if (req.method === "HEAD" && c && c !== "0") {
      throw Error("http: method cannot contain a Content-Length");
    }
    if (c && req.headers.has("transfer-encoding")) {
      // A sender MUST NOT send a Content-Length header field in any message
      // that contains a Transfer-Encoding header field.
      // rfc: https://tools.ietf.org/html/rfc7230#section-3.3.2
      throw new Error(
        "http: Transfer-Encoding and Content-Length cannot be send together"
      );
    }
  }
}
