import { BufReader, BufWriter } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { assert } from "../testing/asserts.ts";
import { encoder, encode } from "../encoding/utf8.ts";
import { ServerResponse, ServerRequest } from "./server.ts";
import { STATUS_TEXT } from "./http_status.ts";
import { letTimeout, timeoutReader } from "../async/timeout.ts";
import { bytesReader } from "../io/readers.ts";
import { ClientResponse, ClientRequest } from "./client.ts";
import { readUntilEOF } from "../io/ioutil.ts";

/** Reader for HTTP/1.1 fixed size body part */
export function bodyReader(contentLength: number, r: BufReader): Deno.Reader {
  let totalRead = 0;
  let finished = false;
  async function read(buf: Uint8Array): Promise<number | null> {
    if (finished) return null;
    let result: number | null;
    const remaining = contentLength - totalRead;
    if (remaining >= buf.byteLength) {
      result = await r.read(buf);
    } else {
      const readBuf = buf.subarray(0, remaining);
      result = await r.read(readBuf);
    }
    if (result !== null) {
      totalRead += result;
    }
    finished = totalRead === contentLength;
    return result;
  }
  return { read };
}

/** Reader for HTTP/1.1 chunked body part */
export function chunkedBodyReader(h: Headers, r: BufReader): Deno.Reader {
  // Based on https://tools.ietf.org/html/rfc2616#section-19.4.6
  const tp = new TextProtoReader(r);
  let finished = false;
  const chunks: Array<{
    offset: number;
    data: Uint8Array;
  }> = [];
  async function read(buf: Uint8Array): Promise<number | null> {
    if (finished) return null;
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
        if ((await tp.readLine()) === null) {
          throw new Deno.errors.UnexpectedEof();
        }
      }
      return readLength;
    }
    const line = await tp.readLine();
    if (line === null) throw new Deno.errors.UnexpectedEof();
    // TODO: handle chunk extension
    const [chunkSizeString] = line.split(";");
    const chunkSize = parseInt(chunkSizeString, 16);
    if (Number.isNaN(chunkSize) || chunkSize < 0) {
      throw new Error("Invalid chunk size");
    }
    if (chunkSize > 0) {
      if (chunkSize > buf.byteLength) {
        let eof = await r.readFull(buf);
        if (eof === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        const restChunk = new Uint8Array(chunkSize - buf.byteLength);
        eof = await r.readFull(restChunk);
        if (eof === null) {
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
        if (eof === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        // Consume \r\n
        if ((await tp.readLine()) === null) {
          throw new Deno.errors.UnexpectedEof();
        }
        return chunkSize;
      }
    } else {
      assert(chunkSize === 0);
      // Consume \r\n
      if ((await r.readLine()) === null) {
        throw new Deno.errors.UnexpectedEof();
      }
      await readTrailers(h, r);
      finished = true;
      return null;
    }
  }
  return { read };
}

function isProhibidedForTrailer(key: string): boolean {
  const s = new Set(["transfer-encoding", "content-length", "trailer"]);
  return s.has(key.toLowerCase());
}

/**
 * Read trailer headers from reader and append values to headers.
 * "trailer" field will be deleted.
 * */
export async function readTrailers(
  headers: Headers,
  r: BufReader
): Promise<void> {
  const headerKeys = parseTrailer(headers.get("trailer"));
  if (!headerKeys) return;
  const tp = new TextProtoReader(r);
  const result = await tp.readMIMEHeader();
  assert(result !== null, "trailer must be set");
  for (const [k, v] of result) {
    if (!headerKeys.has(k)) {
      throw new Error("Undeclared trailer field");
    }
    headerKeys.delete(k);
    headers.append(k, v);
  }
  assert(Array.from(headerKeys).length === 0, "Missing trailers");
  headers.delete("trailer");
}

function parseTrailer(field: string | null): Headers | undefined {
  if (field == null) {
    return undefined;
  }
  const keys = field.split(",").map((v) => v.trim().toLowerCase());
  if (keys.length === 0) {
    throw new Error("Empty trailer");
  }
  for (const key of keys) {
    if (isProhibidedForTrailer(key)) {
      throw new Error(`Prohibited field for trailer`);
    }
  }
  return new Headers(keys.map((key) => [key, ""]));
}

export async function writeChunkedBody(
  w: Deno.Writer,
  r: Deno.Reader
): Promise<void> {
  const writer = BufWriter.create(w);
  for await (const chunk of Deno.iter(r)) {
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
    throw new Error('headers must have "trailer" header field');
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
      !isProhibidedForTrailer(f),
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

export async function readResponse(
  r: Deno.Reader,
  { timeout }: { timeout?: number } = {}
): Promise<ClientResponse> {
  const reader = BufReader.create(r);
  const tp = new TextProtoReader(reader);
  // First line: HTTP/1,1 200 OK
  const line = await letTimeout(tp.readLine(), timeout);
  if (line === null) {
    throw Deno.errors.UnexpectedEof;
  }
  const [proto, status, statusText] = line.split(" ", 3);
  const headers = await letTimeout(tp.readMIMEHeader(), timeout);
  if (headers === null) {
    throw Deno.errors.UnexpectedEof;
  }
  const contentLength = headers.get("content-length");
  const isChunked = headers.get("transfer-encoding")?.match(/^chunked/);
  let body: Deno.Reader;
  if (isChunked) {
    body = chunkedBodyReader(headers, reader);
  } else if (contentLength != null) {
    body = bodyReader(parseInt(contentLength), reader);
  } else {
    throw new Error("No conetnt-length or unknown transfer-encoding");
  }
  if (timeout != null) {
    body = timeoutReader(body, timeout);
  }
  let finalized = false;
  const finalize = async (): Promise<void> => {
    if (finalized) return;
    await readUntilEOF(body);
    finalized = true;
  };
  return {
    proto,
    status: parseInt(status),
    statusText,
    headers,
    body,
    finalize,
  };
}

export async function writeResponse(
  w: Deno.Writer,
  r: ServerResponse
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

  const headers = r.headers ?? new Headers();

  if (r.body && !headers.get("content-length")) {
    if (r.body instanceof Uint8Array) {
      out += `content-length: ${r.body.byteLength}\r\n`;
    } else if (!headers.get("transfer-encoding")) {
      out += "transfer-encoding: chunked\r\n";
    }
  }

  for (const [key, value] of headers) {
    out += `${key}: ${value}\r\n`;
  }

  out += `\r\n`;

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
    const n = await Deno.copy(r.body, writer);
    assert(n === bodyLength);
  } else {
    await writeChunkedBody(writer, r.body);
  }
  await writer.flush();
  if (r.trailers) {
    const t = await r.trailers();
    await writeTrailers(writer, headers, t);
  }
}

/**
 * ParseHTTPVersion parses a HTTP version string.
 * "HTTP/1.0" returns (1, 0).
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

      if (!vers.startsWith("HTTP/")) {
        break;
      }

      const dot = vers.indexOf(".");
      if (dot < 0) {
        break;
      }

      const majorStr = vers.substring(vers.indexOf("/") + 1, dot);
      const major = Number(majorStr);
      if (!Number.isInteger(major) || major < 0 || major > Big) {
        break;
      }

      const minorStr = vers.substring(dot + 1);
      const minor = Number(minorStr);
      if (!Number.isInteger(minor) || minor < 0 || minor > Big) {
        break;
      }

      return [major, minor];
    }
  }

  throw new Error(`malformed HTTP version ${vers}`);
}

/** Read HTTP/1.1 request line and */
export async function readRequest(
  conn: Deno.Conn,
  opts?: {
    r?: BufReader;
    w?: BufWriter;
    timeout?: number; // ms
  }
): Promise<ServerRequest | null> {
  const r = opts?.r ?? new BufReader(conn);
  const w = opts?.w ?? new BufWriter(conn);
  const tp = new TextProtoReader(r);
  const timeout = opts?.timeout;
  // e.g. GET /index.html HTTP/1.0
  const firstLine = await letTimeout(tp.readLine(), timeout);
  if (firstLine === null) return null;
  const headers = await letTimeout(tp.readMIMEHeader(), timeout);
  if (headers === null) throw new Deno.errors.UnexpectedEof();
  const [method, url, proto] = firstLine.split(" ", 3);
  assert(
    method != null && url != null && proto != null,
    "Invalid request line"
  );
  fixLength(method, headers);
  return new ServerRequest({
    url,
    proto,
    method,
    headers,
    w,
    r,
    conn,
    timeout,
  });
}

export async function writeRequest(
  w: Deno.Writer,
  req: ClientRequest
): Promise<void> {
  const url = new URL(req.url);
  const headers = req.headers ?? new Headers();
  if (!headers.has("host")) {
    headers.set("host", url.hostname);
  }
  let pathname = url.pathname;
  const query = url.searchParams.toString();
  if (query) {
    pathname += `?${query}`;
  }
  const lines: string[] = [`${req.method} ${pathname} HTTP/1.1`];
  let body: Deno.Reader | undefined;
  let contentLength: number | undefined;
  if (req.body) {
    [body, contentLength] = setupBody(headers, req.body);
  }
  fixLength(req.method, headers);
  for (const [k, v] of headers) {
    lines.push(`${k}: ${v}`);
  }
  lines.push("\r\n");
  const bufw = BufWriter.create(w);
  await bufw.write(encode(lines.join("\r\n")));
  await bufw.flush();
  if (body) {
    if (contentLength == null) {
      await writeChunkedBody(bufw, body);
    } else {
      await Deno.copy(body, bufw);
    }
    await bufw.flush();
  }
  if (req.trailers) {
    await writeTrailers(bufw, headers, await req.trailers());
  }
}

export function setupBody(
  headers: Headers,
  body: string | Uint8Array | Deno.Reader
): [Deno.Reader, number | undefined] {
  const [r, len] = bodyToReader(body, headers);
  const transferEncoding = headers.get("transfer-encoding");
  let chunked = transferEncoding?.match(/^chunked/) != null;
  if (!chunked && typeof len === "number") {
    headers.set("content-length", `${len}`);
  }
  if (typeof body === "string") {
    if (!headers.has("content-type")) {
      headers.set("content-type", "text/plain; charset=UTF-8");
    }
  } else if (body instanceof Uint8Array) {
    // noop
  } else {
    if (!headers.has("content-length") && !headers.has("transfer-encoding")) {
      headers.set("transfer-encoding", "chunked");
      chunked = true;
    }
  }
  if (!headers.has("content-type")) {
    headers.set("content-type", "application/octet-stream");
  }
  if (chunked) {
    headers.delete("content-length");
  } else {
    headers.delete("transfer-encoding");
  }
  return [r, chunked ? undefined : len];
}

function bodyToReader(
  body: string | Uint8Array | Deno.Reader,
  headers: Headers
): [Deno.Reader, number | undefined] {
  if (typeof body === "string") {
    const bin = encode(body);
    return [bytesReader(bin), bin.byteLength];
  } else if (body instanceof Uint8Array) {
    return [bytesReader(body), body.byteLength];
  } else {
    const cl = headers.get("content-length");
    if (cl) {
      return [body, parseInt(cl)];
    }
    return [body, undefined];
  }
}

function fixLength(method: string, headers: Headers): void {
  const contentLength = headers.get("Content-Length");
  if (contentLength) {
    const arrClen = contentLength.split(",");
    if (arrClen.length > 1) {
      const distinct = [...new Set(arrClen.map((e): string => e.trim()))];
      if (distinct.length > 1) {
        throw Error("cannot contain multiple Content-Length headers");
      } else {
        headers.set("Content-Length", distinct[0]);
      }
    }
    const c = headers.get("Content-Length");
    if (method === "HEAD" && c && c !== "0") {
      throw Error("http: method cannot contain a Content-Length");
    }
    if (c && headers.has("transfer-encoding")) {
      // A sender MUST NOT send a Content-Length header field in any message
      // that contains a Transfer-Encoding header field.
      // rfc: https://tools.ietf.org/html/rfc7230#section-3.3.2
      throw new Error(
        "http: Transfer-Encoding and Content-Length cannot be send together"
      );
    }
  }
}

export type KeepAlive = {
  timeout?: number;
  max?: number;
};

export function parseKeepAlive(value: string): KeepAlive {
  const result: KeepAlive = {};
  const kv = value.split(",").map((s) => s.trim().split("="));
  for (const [key, value] of kv) {
    if (key === "timeout") {
      result.timeout = parseInt(value);
    } else if (key === "max") {
      result.max = parseInt(value);
    }
  }
  return result;
}
