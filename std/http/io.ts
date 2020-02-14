import { BufReader, UnexpectedEOFError, BufWriter } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { assert } from "../testing/asserts.ts";
import { encoder } from "../strings/mod.ts";

export function emptyReader(): Deno.Reader {
  return {
    async read(_: Uint8Array): Promise<number | Deno.EOF> {
      return Deno.EOF;
    }
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
          throw new UnexpectedEOFError();
        }
      }
      return readLength;
    }
    const line = await tp.readLine();
    if (line === Deno.EOF) throw new UnexpectedEOFError();
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
          throw new UnexpectedEOFError();
        }
        const restChunk = new Uint8Array(chunkSize - buf.byteLength);
        eof = await r.readFull(restChunk);
        if (eof === Deno.EOF) {
          throw new UnexpectedEOFError();
        } else {
          chunks.push({
            offset: 0,
            data: restChunk
          });
        }
        return buf.byteLength;
      } else {
        const bufToFill = buf.subarray(0, chunkSize);
        const eof = await r.readFull(bufToFill);
        if (eof === Deno.EOF) {
          throw new UnexpectedEOFError();
        }
        // Consume \r\n
        if ((await tp.readLine()) === Deno.EOF) {
          throw new UnexpectedEOFError();
        }
        return chunkSize;
      }
    } else {
      assert(chunkSize === 0);
      // Consume \r\n
      if ((await r.readLine()) === Deno.EOF) {
        throw new UnexpectedEOFError();
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
  "trailer"
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
  const keys = field.split(",").map(v => v.trim());
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
    .map(s => s.trim().toLowerCase());
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
