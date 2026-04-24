// Copyright 2018-2026 the Deno authors. MIT license.

// Hand-rolled protobuf3 encoder/decoder for the Deno KV Connect protocol.
// No external dependencies. Implements only the wire format primitives needed
// for the datapath messages defined in com.deno.kv.datapath.

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypePush,
  BigInt,
  Error,
  Number,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeSubarray,
  Uint8Array,
} = primordials;

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
      ArrayPrototypePush(bytes, byte);
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
    ArrayPrototypePush(bytes, byte);
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
): { value: number; offset: number } {
  let result = 0;
  let shift = 0;
  let pos = offset;
  const len = TypedArrayPrototypeGetLength(buf);
  while (pos < len) {
    const byte = buf[pos++];
    result += (byte & 0x7F) * (2 ** shift); // avoid bitwise to support >32 bits
    shift += 7;
    if ((byte & 0x80) === 0) {
      return { value: result, offset: pos };
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
): { value: bigint; offset: number } {
  let result = 0n;
  let shift = 0n;
  let pos = offset;
  const len = TypedArrayPrototypeGetLength(buf);
  while (pos < len) {
    const byte = buf[pos++];
    result |= BigInt(byte & 0x7F) << shift;
    shift += 7n;
    if ((byte & 0x80) === 0) {
      return { value: result, offset: pos };
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
  return concatArrays([tag, data]);
}

/** Encode a length-delimited (wire type 2) field: tag + length + data. */
export function encodeBytes(fieldNum: number, data: Uint8Array): Uint8Array {
  const tag = encodeVarint((fieldNum << 3) | WIRE_LENGTH_DELIMITED);
  const len = encodeVarint(TypedArrayPrototypeGetLength(data));
  return concatArrays([tag, len, data]);
}

/** Encode a varint (wire type 0) field: tag + varint value. */
export function encodeVarintField(
  fieldNum: number,
  value: number | bigint,
): Uint8Array {
  const tag = encodeVarint((fieldNum << 3) | WIRE_VARINT);
  const val = encodeVarint(value);
  return concatArrays([tag, val]);
}

/** Concatenate multiple Uint8Arrays into one. */
export function concat(a: Uint8Array, b: Uint8Array): Uint8Array {
  const aLen = TypedArrayPrototypeGetLength(a);
  const bLen = TypedArrayPrototypeGetLength(b);
  const result = new Uint8Array(aLen + bLen);
  TypedArrayPrototypeSet(result, a, 0);
  TypedArrayPrototypeSet(result, b, aLen);
  return result;
}

/** Concatenate an array of Uint8Arrays into one. */
function concatArrays(arrays: Uint8Array[]): Uint8Array {
  let totalLen = 0;
  for (let i = 0; i < arrays.length; i++) {
    totalLen += TypedArrayPrototypeGetLength(arrays[i]);
  }
  const result = new Uint8Array(totalLen);
  let offset = 0;
  for (let i = 0; i < arrays.length; i++) {
    TypedArrayPrototypeSet(result, arrays[i], offset);
    offset += TypedArrayPrototypeGetLength(arrays[i]);
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
  for (let i = 0; i < values.length; i++) {
    ArrayPrototypePush(parts, encodeVarint(values[i]));
  }
  return encodeBytes(fieldNum, concatArrays(parts));
}

// ---------------------------------------------------------------------------
// Individual message encoders
// ---------------------------------------------------------------------------

function encodeReadRange(r: ReadRange): Uint8Array {
  const parts: Uint8Array[] = [];
  if (TypedArrayPrototypeGetLength(r.start) > 0) {
    ArrayPrototypePush(parts, encodeBytes(1, r.start));
  }
  if (TypedArrayPrototypeGetLength(r.end) > 0) {
    ArrayPrototypePush(parts, encodeBytes(2, r.end));
  }
  if (r.limit !== 0) ArrayPrototypePush(parts, encodeVarintField(3, r.limit));
  if (r.reverse) ArrayPrototypePush(parts, encodeVarintField(4, 1));
  return concatArrays(parts);
}

function encodeCheck(c: Check): Uint8Array {
  const parts: Uint8Array[] = [];
  if (TypedArrayPrototypeGetLength(c.key) > 0) {
    ArrayPrototypePush(parts, encodeBytes(1, c.key));
  }
  if (TypedArrayPrototypeGetLength(c.versionstamp) > 0) {
    ArrayPrototypePush(parts, encodeBytes(2, c.versionstamp));
  }
  return concatArrays(parts);
}

function encodeKvValue(v: KvValue): Uint8Array {
  const parts: Uint8Array[] = [];
  if (TypedArrayPrototypeGetLength(v.data) > 0) {
    ArrayPrototypePush(parts, encodeBytes(1, v.data));
  }
  if (v.encoding !== 0) {
    ArrayPrototypePush(parts, encodeVarintField(2, v.encoding));
  }
  return concatArrays(parts);
}

function encodeMutation(m: Mutation): Uint8Array {
  const parts: Uint8Array[] = [];
  if (TypedArrayPrototypeGetLength(m.key) > 0) {
    ArrayPrototypePush(parts, encodeBytes(1, m.key));
  }
  if (m.value !== null) {
    ArrayPrototypePush(parts, encodeMessage(2, encodeKvValue(m.value)));
  }
  if (m.mutationType !== 0) {
    ArrayPrototypePush(parts, encodeVarintField(3, m.mutationType));
  }
  const expireMs = toBigInt(m.expireAtMs);
  if (expireMs !== 0n) {
    ArrayPrototypePush(parts, encodeVarintField(4, expireMs));
  }
  if (TypedArrayPrototypeGetLength(m.sumMin) > 0) {
    ArrayPrototypePush(parts, encodeBytes(5, m.sumMin));
  }
  if (TypedArrayPrototypeGetLength(m.sumMax) > 0) {
    ArrayPrototypePush(parts, encodeBytes(6, m.sumMax));
  }
  if (m.sumClamp) ArrayPrototypePush(parts, encodeVarintField(7, 1));
  return concatArrays(parts);
}

function encodeEnqueue(e: Enqueue): Uint8Array {
  const parts: Uint8Array[] = [];
  if (TypedArrayPrototypeGetLength(e.payload) > 0) {
    ArrayPrototypePush(parts, encodeBytes(1, e.payload));
  }
  const deadlineMs = toBigInt(e.deadlineMs);
  if (deadlineMs !== 0n) {
    ArrayPrototypePush(parts, encodeVarintField(2, deadlineMs));
  }
  for (let i = 0; i < e.keysIfUndelivered.length; i++) {
    ArrayPrototypePush(parts, encodeBytes(3, e.keysIfUndelivered[i]));
  }
  const packed = encodePackedUint32(4, e.backoffSchedule);
  if (TypedArrayPrototypeGetLength(packed) > 0) {
    ArrayPrototypePush(parts, packed);
  }
  return concatArrays(parts);
}

function encodeWatchKey(k: WatchKey): Uint8Array {
  const parts: Uint8Array[] = [];
  if (TypedArrayPrototypeGetLength(k.key) > 0) {
    ArrayPrototypePush(parts, encodeBytes(1, k.key));
  }
  return concatArrays(parts);
}

// ---------------------------------------------------------------------------
// High-level encoders (client -> server)
// ---------------------------------------------------------------------------

/** Encode a SnapshotRead message. */
export function encodeSnapshotRead(ranges: ReadRange[]): Uint8Array {
  const parts: Uint8Array[] = [];
  for (let i = 0; i < ranges.length; i++) {
    ArrayPrototypePush(parts, encodeMessage(1, encodeReadRange(ranges[i])));
  }
  return concatArrays(parts);
}

/** Encode an AtomicWrite message. */
export function encodeAtomicWrite(
  write: { checks: Check[]; mutations: Mutation[]; enqueues: Enqueue[] },
): Uint8Array {
  const parts: Uint8Array[] = [];
  for (let i = 0; i < write.checks.length; i++) {
    ArrayPrototypePush(parts, encodeMessage(1, encodeCheck(write.checks[i])));
  }
  for (let i = 0; i < write.mutations.length; i++) {
    ArrayPrototypePush(
      parts,
      encodeMessage(2, encodeMutation(write.mutations[i])),
    );
  }
  for (let i = 0; i < write.enqueues.length; i++) {
    ArrayPrototypePush(
      parts,
      encodeMessage(3, encodeEnqueue(write.enqueues[i])),
    );
  }
  return concatArrays(parts);
}

/** Encode a Watch message. */
export function encodeWatch(keys: Uint8Array[]): Uint8Array {
  const parts: Uint8Array[] = [];
  for (let i = 0; i < keys.length; i++) {
    ArrayPrototypePush(
      parts,
      encodeMessage(1, encodeWatchKey({ key: keys[i] })),
    );
  }
  return concatArrays(parts);
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
 * Parse all fields in a protobuf message buffer.
 * Returns an array of WireField objects with the appropriate field populated
 * based on the wire type.
 */
function parseFields(buf: Uint8Array): WireField[] {
  const fields: WireField[] = [];
  let offset = 0;
  const bufLen = TypedArrayPrototypeGetLength(buf);
  while (offset < bufLen) {
    const tagResult = decodeVarint(buf, offset);
    offset = tagResult.offset;
    const wireType = tagResult.value & 0x07;
    const fieldNum = tagResult.value >>> 3;

    if (wireType === WIRE_VARINT) {
      // Decode varint value - we provide both number and bigint forms
      const valResult = decodeVarint(buf, offset);
      const val64Result = decodeVarint64(buf, offset);
      offset = valResult.offset;
      ArrayPrototypePush(fields, {
        fieldNum,
        wireType,
        varintValue: valResult.value,
        varint64Value: val64Result.value,
        data: EMPTY,
      });
    } else if (wireType === WIRE_LENGTH_DELIMITED) {
      const lenResult = decodeVarint(buf, offset);
      offset = lenResult.offset;
      const data = TypedArrayPrototypeSubarray(
        buf,
        offset,
        offset + lenResult.value,
      );
      offset += lenResult.value;
      ArrayPrototypePush(fields, {
        fieldNum,
        wireType,
        varintValue: 0,
        varint64Value: 0n,
        data,
      });
    } else if (wireType === 5) {
      // 32-bit (fixed32, sfixed32, float) - skip 4 bytes
      offset += 4;
      ArrayPrototypePush(fields, {
        fieldNum,
        wireType,
        varintValue: 0,
        varint64Value: 0n,
        data: EMPTY,
      });
    } else if (wireType === 1) {
      // 64-bit (fixed64, sfixed64, double) - skip 8 bytes
      offset += 8;
      ArrayPrototypePush(fields, {
        fieldNum,
        wireType,
        varintValue: 0,
        varint64Value: 0n,
        data: EMPTY,
      });
    } else {
      throw new Error(`protobuf: unsupported wire type ${wireType}`);
    }
  }
  return fields;
}

/**
 * Decode a packed repeated uint32 field.
 * The data is a sequence of varints packed into a length-delimited blob.
 */
function decodePackedUint32(data: Uint8Array): number[] {
  const values: number[] = [];
  let offset = 0;
  const len = TypedArrayPrototypeGetLength(data);
  while (offset < len) {
    const r = decodeVarint(data, offset);
    ArrayPrototypePush(values, r.value);
    offset = r.offset;
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
  const fields = parseFields(buf);
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
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
  const fields = parseFields(buf);
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
    if (f.fieldNum === 1) {
      ArrayPrototypePush(output.values, decodeKvEntry(f.data));
    }
  }
  return output;
}

function decodeWatchKeyOutput(buf: Uint8Array): WatchKeyOutput {
  const output: WatchKeyOutput = {
    changed: false,
    entryIfChanged: null,
  };
  const fields = parseFields(buf);
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
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
  const fields = parseFields(buf);
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
    switch (f.fieldNum) {
      case 1:
        ArrayPrototypePush(output.ranges, decodeReadRangeOutput(f.data));
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
  const fields = parseFields(buf);
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
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
          const packed = decodePackedUint32(f.data);
          for (let j = 0; j < packed.length; j++) {
            ArrayPrototypePush(output.failedChecks, packed[j]);
          }
        } else {
          ArrayPrototypePush(output.failedChecks, f.varintValue);
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
  const fields = parseFields(buf);
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
    switch (f.fieldNum) {
      case 1:
        output.status = f.varintValue as SnapshotReadStatus;
        break;
      case 2:
        ArrayPrototypePush(output.keys, decodeWatchKeyOutput(f.data));
        break;
    }
  }
  return output;
}
