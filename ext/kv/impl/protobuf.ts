// Copyright 2018-2026 the Deno authors. All rights reserved. MIT license.

// Hand-rolled protobuf3 encoder/decoder for the Deno KV Connect protocol.
// No external dependencies. Implements only the wire format primitives needed
// for the datapath messages defined in com.deno.kv.datapath.

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

export const SnapshotReadStatus = {
  SR_UNSPECIFIED: 0,
  SR_SUCCESS: 1,
  SR_READ_DISABLED: 2,
} as const;
export type SnapshotReadStatus =
  (typeof SnapshotReadStatus)[keyof typeof SnapshotReadStatus];

export const MutationType = {
  M_UNSPECIFIED: 0,
  M_SET: 1,
  M_DELETE: 2,
  M_SUM: 3,
  M_MAX: 4,
  M_MIN: 5,
  M_SET_SUFFIX_VERSIONSTAMPED_KEY: 9,
} as const;
export type MutationType = (typeof MutationType)[keyof typeof MutationType];

export const ValueEncoding = {
  VE_UNSPECIFIED: 0,
  VE_V8: 1,
  VE_LE64: 2,
  VE_BYTES: 3,
} as const;
export type ValueEncoding = (typeof ValueEncoding)[keyof typeof ValueEncoding];

export const AtomicWriteStatus = {
  AW_UNSPECIFIED: 0,
  AW_SUCCESS: 1,
  AW_CHECK_FAILURE: 2,
  AW_WRITE_DISABLED: 5,
} as const;
export type AtomicWriteStatus =
  (typeof AtomicWriteStatus)[keyof typeof AtomicWriteStatus];

// ---------------------------------------------------------------------------
// Interfaces
// ---------------------------------------------------------------------------

export interface ReadRange {
  start: Uint8Array;
  end: Uint8Array;
  limit: number;
  reverse: boolean;
}

export interface ReadRangeOutput {
  values: KvEntry[];
}

export interface SnapshotRead {
  ranges: ReadRange[];
}

export interface SnapshotReadOutput {
  ranges: ReadRangeOutput[];
  readDisabled: boolean;
  readIsStronglyConsistent: boolean;
  status: SnapshotReadStatus;
}

export interface Check {
  key: Uint8Array;
  versionstamp: Uint8Array;
}

export interface KvValue {
  data: Uint8Array;
  encoding: ValueEncoding;
}

export interface Mutation {
  key: Uint8Array;
  value: KvValue | null;
  mutationType: MutationType;
  expireAtMs: bigint | number;
  sumMin: Uint8Array;
  sumMax: Uint8Array;
  sumClamp: boolean;
}

export interface KvEntry {
  key: Uint8Array;
  value: Uint8Array;
  encoding: ValueEncoding;
  versionstamp: Uint8Array;
}

export interface Enqueue {
  payload: Uint8Array;
  deadlineMs: bigint | number;
  keysIfUndelivered: Uint8Array[];
  backoffSchedule: number[];
}

export interface AtomicWrite {
  checks: Check[];
  mutations: Mutation[];
  enqueues: Enqueue[];
}

export interface AtomicWriteOutput {
  status: AtomicWriteStatus;
  versionstamp: Uint8Array;
  failedChecks: number[];
}

export interface WatchKey {
  key: Uint8Array;
}

export interface Watch {
  keys: WatchKey[];
}

export interface WatchKeyOutput {
  changed: boolean;
  entryIfChanged: KvEntry | null;
}

export interface WatchOutput {
  status: SnapshotReadStatus;
  keys: WatchKeyOutput[];
}

// ---------------------------------------------------------------------------
// Wire format constants
// ---------------------------------------------------------------------------

const WIRE_VARINT = 0;
const WIRE_LENGTH_DELIMITED = 2;

const EMPTY = new Uint8Array(0);

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

/** Encode a non-negative integer or bigint as an unsigned LEB128 varint. */
export function encodeVarint(n: number | bigint): Uint8Array {
  if (typeof n === "number") {
    if (n < 0) {
      // Encode negative numbers as 10-byte two's complement
      return encodeVarint(BigInt(n) & 0xFFFFFFFFFFFFFFFFn);
    }
    // Fast path for small values
    if (n === 0) return new Uint8Array([0]);
    const bytes: number[] = [];
    while (n > 0) {
      let byte = n & 0x7F;
      n >>>= 7;
      if (n > 0) byte |= 0x80;
      bytes.push(byte);
    }
    return new Uint8Array(bytes);
  }
  // BigInt path
  let v = n;
  if (v < 0n) {
    v = v & 0xFFFFFFFFFFFFFFFFn;
  }
  if (v === 0n) return new Uint8Array([0]);
  const bytes: number[] = [];
  while (v > 0n) {
    let byte = Number(v & 0x7Fn);
    v >>= 7n;
    if (v > 0n) byte |= 0x80;
    bytes.push(byte);
  }
  return new Uint8Array(bytes);
}

/**
 * Decode an unsigned varint from buf starting at offset.
 * Returns [value, newOffset]. Suitable for values that fit in a safe integer
 * (up to 2^53-1), which covers all uint32, int32, enum, and bool fields.
 */
export function decodeVarint(
  buf: Uint8Array,
  offset: number,
): [number, number] {
  let result = 0;
  let shift = 0;
  let pos = offset;
  while (pos < buf.length) {
    const byte = buf[pos++];
    result += (byte & 0x7F) * (2 ** shift); // avoid bitwise to support >32 bits
    shift += 7;
    if ((byte & 0x80) === 0) {
      return [result, pos];
    }
  }
  throw new Error("protobuf: unterminated varint");
}

/**
 * Decode an unsigned 64-bit varint as a BigInt.
 * Used for int64 / uint64 fields.
 */
export function decodeVarint64(
  buf: Uint8Array,
  offset: number,
): [bigint, number] {
  let result = 0n;
  let shift = 0n;
  let pos = offset;
  while (pos < buf.length) {
    const byte = buf[pos++];
    result |= BigInt(byte & 0x7F) << shift;
    shift += 7n;
    if ((byte & 0x80) === 0) {
      return [result, pos];
    }
  }
  throw new Error("protobuf: unterminated varint");
}

/** Encode a single protobuf field: tag + data. */
export function encodeField(
  fieldNum: number,
  wireType: number,
  data: Uint8Array,
): Uint8Array {
  const tag = encodeVarint((fieldNum << 3) | wireType);
  return concat(tag, data);
}

/** Encode a length-delimited (wire type 2) field: tag + length + data. */
export function encodeBytes(fieldNum: number, data: Uint8Array): Uint8Array {
  const tag = encodeVarint((fieldNum << 3) | WIRE_LENGTH_DELIMITED);
  const len = encodeVarint(data.length);
  return concat(tag, len, data);
}

/** Encode a varint (wire type 0) field: tag + varint value. */
export function encodeVarintField(
  fieldNum: number,
  value: number | bigint,
): Uint8Array {
  const tag = encodeVarint((fieldNum << 3) | WIRE_VARINT);
  const val = encodeVarint(value);
  return concat(tag, val);
}

/** Concatenate multiple Uint8Arrays into one. */
export function concat(...arrays: Uint8Array[]): Uint8Array {
  let totalLen = 0;
  for (let i = 0; i < arrays.length; i++) {
    totalLen += arrays[i].length;
  }
  const result = new Uint8Array(totalLen);
  let offset = 0;
  for (let i = 0; i < arrays.length; i++) {
    result.set(arrays[i], offset);
    offset += arrays[i].length;
  }
  return result;
}

// ---------------------------------------------------------------------------
// Internal encoding helpers
// ---------------------------------------------------------------------------

function toBigInt(v: bigint | number): bigint {
  return typeof v === "number" ? BigInt(v) : v;
}

/** Encode a sub-message as a length-delimited field. */
function encodeMessage(fieldNum: number, inner: Uint8Array): Uint8Array {
  return encodeBytes(fieldNum, inner);
}

/**
 * Encode a packed repeated uint32 field.
 * Proto3 uses packed encoding by default for repeated scalar fields.
 */
function encodePackedUint32(fieldNum: number, values: number[]): Uint8Array {
  if (values.length === 0) return EMPTY;
  const parts: Uint8Array[] = [];
  for (const v of values) {
    parts.push(encodeVarint(v));
  }
  return encodeBytes(fieldNum, concat(...parts));
}

// ---------------------------------------------------------------------------
// Individual message encoders
// ---------------------------------------------------------------------------

function encodeReadRange(r: ReadRange): Uint8Array {
  const parts: Uint8Array[] = [];
  if (r.start.length > 0) parts.push(encodeBytes(1, r.start));
  if (r.end.length > 0) parts.push(encodeBytes(2, r.end));
  if (r.limit !== 0) parts.push(encodeVarintField(3, r.limit));
  if (r.reverse) parts.push(encodeVarintField(4, 1));
  return concat(...parts);
}

function encodeCheck(c: Check): Uint8Array {
  const parts: Uint8Array[] = [];
  if (c.key.length > 0) parts.push(encodeBytes(1, c.key));
  if (c.versionstamp.length > 0) parts.push(encodeBytes(2, c.versionstamp));
  return concat(...parts);
}

function encodeKvValue(v: KvValue): Uint8Array {
  const parts: Uint8Array[] = [];
  if (v.data.length > 0) parts.push(encodeBytes(1, v.data));
  if (v.encoding !== 0) parts.push(encodeVarintField(2, v.encoding));
  return concat(...parts);
}

function encodeMutation(m: Mutation): Uint8Array {
  const parts: Uint8Array[] = [];
  if (m.key.length > 0) parts.push(encodeBytes(1, m.key));
  if (m.value !== null) {
    parts.push(encodeMessage(2, encodeKvValue(m.value)));
  }
  if (m.mutationType !== 0) parts.push(encodeVarintField(3, m.mutationType));
  const expireMs = toBigInt(m.expireAtMs);
  if (expireMs !== 0n) parts.push(encodeVarintField(4, expireMs));
  if (m.sumMin.length > 0) parts.push(encodeBytes(5, m.sumMin));
  if (m.sumMax.length > 0) parts.push(encodeBytes(6, m.sumMax));
  if (m.sumClamp) parts.push(encodeVarintField(7, 1));
  return concat(...parts);
}

function encodeEnqueue(e: Enqueue): Uint8Array {
  const parts: Uint8Array[] = [];
  if (e.payload.length > 0) parts.push(encodeBytes(1, e.payload));
  const deadlineMs = toBigInt(e.deadlineMs);
  if (deadlineMs !== 0n) parts.push(encodeVarintField(2, deadlineMs));
  for (const key of e.keysIfUndelivered) {
    parts.push(encodeBytes(3, key));
  }
  const packed = encodePackedUint32(4, e.backoffSchedule);
  if (packed.length > 0) parts.push(packed);
  return concat(...parts);
}

function encodeWatchKey(k: WatchKey): Uint8Array {
  const parts: Uint8Array[] = [];
  if (k.key.length > 0) parts.push(encodeBytes(1, k.key));
  return concat(...parts);
}

// ---------------------------------------------------------------------------
// High-level encoders (client -> server)
// ---------------------------------------------------------------------------

/** Encode a SnapshotRead message. */
export function encodeSnapshotRead(ranges: ReadRange[]): Uint8Array {
  const parts: Uint8Array[] = [];
  for (const r of ranges) {
    parts.push(encodeMessage(1, encodeReadRange(r)));
  }
  return concat(...parts);
}

/** Encode an AtomicWrite message. */
export function encodeAtomicWrite(
  write: { checks: Check[]; mutations: Mutation[]; enqueues: Enqueue[] },
): Uint8Array {
  const parts: Uint8Array[] = [];
  for (const c of write.checks) {
    parts.push(encodeMessage(1, encodeCheck(c)));
  }
  for (const m of write.mutations) {
    parts.push(encodeMessage(2, encodeMutation(m)));
  }
  for (const e of write.enqueues) {
    parts.push(encodeMessage(3, encodeEnqueue(e)));
  }
  return concat(...parts);
}

/** Encode a Watch message. */
export function encodeWatch(keys: Uint8Array[]): Uint8Array {
  const parts: Uint8Array[] = [];
  for (const key of keys) {
    parts.push(encodeMessage(1, encodeWatchKey({ key })));
  }
  return concat(...parts);
}

// ---------------------------------------------------------------------------
// Internal decoding helpers
// ---------------------------------------------------------------------------

/** Parsed wire field: field number, wire type, and raw data. */
interface WireField {
  fieldNum: number;
  wireType: number;
  /** For varint fields this holds the varint value as bytes are consumed. */
  varintValue: number;
  /** For varint64 fields. */
  varint64Value: bigint;
  /** For length-delimited fields. */
  data: Uint8Array;
}

/**
 * Iterate over all fields in a protobuf message buffer.
 * Yields WireField objects with the appropriate field populated
 * based on the wire type.
 */
function* iterFields(buf: Uint8Array): Generator<WireField> {
  let offset = 0;
  while (offset < buf.length) {
    const [tag, newOffset] = decodeVarint(buf, offset);
    offset = newOffset;
    const wireType = tag & 0x07;
    const fieldNum = tag >>> 3;

    if (wireType === WIRE_VARINT) {
      // Decode varint value - we provide both number and bigint forms
      const [val, off2] = decodeVarint(buf, offset);
      const [val64, _off264] = decodeVarint64(buf, offset);
      offset = off2;
      yield {
        fieldNum,
        wireType,
        varintValue: val,
        varint64Value: val64,
        data: EMPTY,
      };
    } else if (wireType === WIRE_LENGTH_DELIMITED) {
      const [len, off2] = decodeVarint(buf, offset);
      offset = off2;
      const data = buf.subarray(offset, offset + len);
      offset += len;
      yield {
        fieldNum,
        wireType,
        varintValue: 0,
        varint64Value: 0n,
        data,
      };
    } else if (wireType === 5) {
      // 32-bit (fixed32, sfixed32, float) - skip 4 bytes
      offset += 4;
      yield {
        fieldNum,
        wireType,
        varintValue: 0,
        varint64Value: 0n,
        data: EMPTY,
      };
    } else if (wireType === 1) {
      // 64-bit (fixed64, sfixed64, double) - skip 8 bytes
      offset += 8;
      yield {
        fieldNum,
        wireType,
        varintValue: 0,
        varint64Value: 0n,
        data: EMPTY,
      };
    } else {
      throw new Error(`protobuf: unsupported wire type ${wireType}`);
    }
  }
}

/**
 * Decode a packed repeated uint32 field.
 * The data is a sequence of varints packed into a length-delimited blob.
 */
function decodePackedUint32(data: Uint8Array): number[] {
  const values: number[] = [];
  let offset = 0;
  while (offset < data.length) {
    const [val, newOffset] = decodeVarint(data, offset);
    values.push(val);
    offset = newOffset;
  }
  return values;
}

// ---------------------------------------------------------------------------
// Individual message decoders
// ---------------------------------------------------------------------------

function decodeKvEntry(buf: Uint8Array): KvEntry {
  const entry: KvEntry = {
    key: EMPTY,
    value: EMPTY,
    encoding: 0 as ValueEncoding,
    versionstamp: EMPTY,
  };
  for (const f of iterFields(buf)) {
    switch (f.fieldNum) {
      case 1:
        entry.key = f.data;
        break;
      case 2:
        entry.value = f.data;
        break;
      case 3:
        entry.encoding = f.varintValue as ValueEncoding;
        break;
      case 4:
        entry.versionstamp = f.data;
        break;
    }
  }
  return entry;
}

function decodeReadRangeOutput(buf: Uint8Array): ReadRangeOutput {
  const output: ReadRangeOutput = { values: [] };
  for (const f of iterFields(buf)) {
    if (f.fieldNum === 1) {
      output.values.push(decodeKvEntry(f.data));
    }
  }
  return output;
}

function decodeWatchKeyOutput(buf: Uint8Array): WatchKeyOutput {
  const output: WatchKeyOutput = {
    changed: false,
    entryIfChanged: null,
  };
  for (const f of iterFields(buf)) {
    switch (f.fieldNum) {
      case 1:
        output.changed = f.varintValue !== 0;
        break;
      case 2:
        output.entryIfChanged = decodeKvEntry(f.data);
        break;
    }
  }
  return output;
}

// ---------------------------------------------------------------------------
// High-level decoders (server -> client)
// ---------------------------------------------------------------------------

/** Decode a SnapshotReadOutput message. */
export function decodeSnapshotReadOutput(buf: Uint8Array): SnapshotReadOutput {
  const output: SnapshotReadOutput = {
    ranges: [],
    readDisabled: false,
    readIsStronglyConsistent: false,
    status: 0 as SnapshotReadStatus,
  };
  for (const f of iterFields(buf)) {
    switch (f.fieldNum) {
      case 1:
        output.ranges.push(decodeReadRangeOutput(f.data));
        break;
      case 2:
        output.readDisabled = f.varintValue !== 0;
        break;
      case 4:
        output.readIsStronglyConsistent = f.varintValue !== 0;
        break;
      case 8:
        output.status = f.varintValue as SnapshotReadStatus;
        break;
    }
  }
  return output;
}

/** Decode an AtomicWriteOutput message. */
export function decodeAtomicWriteOutput(buf: Uint8Array): AtomicWriteOutput {
  const output: AtomicWriteOutput = {
    status: 0 as AtomicWriteStatus,
    versionstamp: EMPTY,
    failedChecks: [],
  };
  for (const f of iterFields(buf)) {
    switch (f.fieldNum) {
      case 1:
        output.status = f.varintValue as AtomicWriteStatus;
        break;
      case 2:
        output.versionstamp = f.data;
        break;
      case 4:
        // Can appear as packed (length-delimited) or individual varints.
        if (f.wireType === WIRE_LENGTH_DELIMITED) {
          output.failedChecks.push(...decodePackedUint32(f.data));
        } else {
          output.failedChecks.push(f.varintValue);
        }
        break;
    }
  }
  return output;
}

/** Decode a WatchOutput message. */
export function decodeWatchOutput(buf: Uint8Array): WatchOutput {
  const output: WatchOutput = {
    status: 0 as SnapshotReadStatus,
    keys: [],
  };
  for (const f of iterFields(buf)) {
    switch (f.fieldNum) {
      case 1:
        output.status = f.varintValue as SnapshotReadStatus;
        break;
      case 2:
        output.keys.push(decodeWatchKeyOutput(f.data));
        break;
    }
  }
  return output;
}
