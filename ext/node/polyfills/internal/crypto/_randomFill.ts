// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  MAX_SIZE as kMaxUint32,
} from "ext:deno_node/internal/crypto/_randomBytes.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
const { core } = globalThis.__bootstrap;
const { ops } = core;

const kBufferMaxLength = 0x7fffffff;

function assertOffset(offset: number, length: number) {
  if (offset > kMaxUint32 || offset < 0) {
    throw new TypeError("offset must be a uint32");
  }

  if (offset > kBufferMaxLength || offset > length) {
    throw new RangeError("offset out of range");
  }
}

function assertSize(size: number, offset: number, length: number) {
  if (size > kMaxUint32 || size < 0) {
    throw new TypeError("size must be a uint32");
  }

  if (size + offset > length || size > kBufferMaxLength) {
    throw new RangeError("buffer too small");
  }
}

export default function randomFill(
  buf: Buffer,
  cb: (err: Error | null, buf: Buffer) => void,
): void;

export default function randomFill(
  buf: Buffer,
  offset: number,
  cb: (err: Error | null, buf: Buffer) => void,
): void;

export default function randomFill(
  buf: Buffer,
  offset: number,
  size: number,
  cb: (err: Error | null, buf: Buffer) => void,
): void;

export default function randomFill(
  buf: Buffer,
  offset?: number | ((err: Error | null, buf: Buffer) => void),
  size?: number | ((err: Error | null, buf: Buffer) => void),
  cb?: (err: Error | null, buf: Buffer) => void,
) {
  if (typeof offset === "function") {
    cb = offset;
    offset = 0;
    size = buf.length;
  } else if (typeof size === "function") {
    cb = size;
    size = buf.length - Number(offset as number);
  }

  assertOffset(offset as number, buf.length);
  assertSize(size as number, offset as number, buf.length);

  core.opAsync("op_node_generate_secret_async", Math.floor(size as number))
    .then(
      (randomData: Uint8Array) => {
        const randomBuf = Buffer.from(randomData.buffer);
        randomBuf.copy(buf, offset as number, 0, size as number);
        cb!(null, buf);
      },
    );
}

export function randomFillSync(buf: Buffer, offset = 0, size?: number) {
  assertOffset(offset, buf.length);

  if (size === undefined) size = buf.length - offset;

  assertSize(size, offset, buf.length);

  const bytes: Uint8Array = new Uint8Array(Math.floor(size));
  ops.op_node_generate_secret(bytes);
  const bytesBuf: Buffer = Buffer.from(bytes.buffer);
  bytesBuf.copy(buf, offset, 0, size);

  return buf;
}
