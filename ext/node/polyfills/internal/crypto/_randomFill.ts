// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import randomBytes, {
  MAX_SIZE as kMaxUint32,
} from "ext:deno_node/internal/crypto/_randomBytes.ts";
import { Buffer } from "ext:deno_node/buffer.ts";

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

  randomBytes(size as number, (err, bytes) => {
    if (err) return cb!(err, buf);
    bytes?.copy(buf, offset as number);
    cb!(null, buf);
  });
}

export function randomFillSync(buf: Buffer, offset = 0, size?: number) {
  assertOffset(offset, buf.length);

  if (size === undefined) size = buf.length - offset;

  assertSize(size, offset, buf.length);

  const bytes = randomBytes(size);

  bytes.copy(buf, offset);

  return buf;
}
