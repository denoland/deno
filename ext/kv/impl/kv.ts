// Copyright 2018-2026 the Deno authors. MIT license.

// Pure JS implementation of the Deno KV API.
// Replaces the Rust ops with JS backends (SQLite for local, HTTP for remote).
//
// NOTE: This module still depends on `core.serialize` / `core.deserialize`
// from deno_core for V8 structured clone of arbitrary JS values.
// That is the only native dependency remaining.

import { core, primordials } from "ext:core/mod.js";
import { op_get_env_no_permission_check } from "ext:core/ops";
const {
  Array,
  ArrayFrom,
  ArrayIsArray,
  ArrayPrototypeMap,
  ArrayPrototypePop,
  ArrayPrototypePush,
  ArrayPrototypeReverse,
  ArrayPrototypeSlice,
  BigInt,
  BigIntPrototypeToString,
  DataView,
  DataViewPrototypeGetBigUint64,
  DataViewPrototypeSetBigUint64,
  DateNow,
  Error,
  MathMin,
  NumberIsNaN,
  Object,
  ObjectAssign,
  ObjectFreeze,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseResolve,
  RangeError,
  ReflectHas,
  RegExpPrototypeTest,
  SafePromiseRace,
  SafeRegExp,
  StringPrototypeCharCodeAt,
  StringPrototypeEndsWith,
  StringPrototypeReplace,
  StringPrototypeStartsWith,
  Symbol,
  SymbolAsyncIterator,
  SymbolDispose,
  SymbolFor,
  SymbolToStringTag,
  TypeError,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeSlice,
  Uint8Array,
  Uint8ArrayPrototype,
} = primordials;

import { ReadableStream } from "ext:deno_web/06_streams.js";

import {
  decodeKey,
  encodeKey,
  keyPartsToKvKey,
  kvKeyToKeyParts,
} from "./key_codec.ts";

import { SqliteBackend } from "./sqlite_backend.ts";
import type {
  Check as SqliteCheck,
  CommitResult,
  Enqueue as SqliteEnqueue,
  KvEntry as SqliteKvEntry,
  KvValue,
  Mutation as SqliteMutation,
  MutationKind,
  ReadRange,
} from "./sqlite_backend.ts";

import { RemoteBackend } from "./remote_backend.ts";
import type { RemoteKvEntry } from "./remote_backend.ts";

import {
  type Check as ProtoCheck,
  type Enqueue as ProtoEnqueue,
  type Mutation as ProtoMutation,
  MutationType,
  type ReadRange as ProtoReadRange,
  ValueEncoding,
} from "./protobuf.ts";

const eqTailRe = new SafeRegExp("=+$");
const versionstampRe = new SafeRegExp("^[0-9a-f]{20}$");

// ---------------------------------------------------------------------------
// Base64url for cursor encoding (matching the Rust backend)
// ---------------------------------------------------------------------------

const BASE64URL =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

function base64urlEncode(data: Uint8Array): string {
  let result = "";
  let i = 0;
  const dataLen = TypedArrayPrototypeGetLength(data);
  while (i < dataLen) {
    const b0 = data[i++];
    const b1 = i < dataLen ? data[i++] : 0;
    const b2 = i < dataLen ? data[i++] : 0;
    const n = (b0 << 16) | (b1 << 8) | b2;
    result += BASE64URL[(n >> 18) & 63];
    result += BASE64URL[(n >> 12) & 63];
    if (i - 1 <= dataLen) result += BASE64URL[(n >> 6) & 63];
    if (i <= dataLen) result += BASE64URL[n & 63];
  }
  // Padding with '='
  const pad = dataLen % 3;
  if (pad === 1) result += "==";
  else if (pad === 2) result += "=";
  return result;
}

function base64urlDecode(str: string): Uint8Array {
  // Count padding to determine exact byte count
  let padding = 0;
  if (StringPrototypeEndsWith(str, "==")) padding = 2;
  else if (StringPrototypeEndsWith(str, "=")) padding = 1;
  const s = StringPrototypeReplace(str, eqTailRe, "");
  const lookup = new Uint8Array(128);
  for (let i = 0; i < BASE64URL.length; i++) {
    lookup[StringPrototypeCharCodeAt(BASE64URL, i)] = i;
  }
  const bytes: number[] = [];
  for (let i = 0; i < s.length; i += 4) {
    const a = lookup[StringPrototypeCharCodeAt(s, i)];
    const b = lookup[StringPrototypeCharCodeAt(s, i + 1)] ?? 0;
    const c = lookup[StringPrototypeCharCodeAt(s, i + 2)] ?? 0;
    const d = lookup[StringPrototypeCharCodeAt(s, i + 3)] ?? 0;
    const n = (a << 18) | (b << 12) | (c << 6) | d;
    ArrayPrototypePush(bytes, (n >> 16) & 0xff);
    ArrayPrototypePush(bytes, (n >> 8) & 0xff);
    ArrayPrototypePush(bytes, n & 0xff);
  }
  // Trim excess bytes from padding
  const exactLength = bytes.length - padding;
  return new Uint8Array(ArrayPrototypeSlice(bytes, 0, exactLength));
}

// ---------------------------------------------------------------------------
// Value serialization - uses V8 structured clone for arbitrary JS values
// ---------------------------------------------------------------------------

interface RawValue {
  kind: "v8" | "bytes" | "u64";
  value: Uint8Array | bigint;
}

function serializeValue(value: unknown): RawValue {
  if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, value)) {
    return { kind: "bytes", value: value as Uint8Array };
  } else if (ObjectPrototypeIsPrototypeOf(KvU64Prototype, value)) {
    return { kind: "u64", value: (value as KvU64).value };
  } else {
    return {
      kind: "v8",
      value: core.serialize(value, { forStorage: true }),
    };
  }
}

function deserializeRawValue(raw: RawValue): unknown {
  switch (raw.kind) {
    case "v8":
      return core.deserialize(raw.value as Uint8Array, { forStorage: true });
    case "bytes":
      return raw.value;
    case "u64":
      return new KvU64(raw.value as bigint);
    default:
      throw new TypeError("Invalid value type");
  }
}

function rawValueToKvValue(raw: RawValue): KvValue {
  switch (raw.kind) {
    case "v8":
      return { kind: "v8", data: raw.value as Uint8Array };
    case "bytes":
      return { kind: "bytes", data: raw.value as Uint8Array };
    case "u64":
      return { kind: "u64", value: raw.value as bigint };
    default:
      throw new TypeError("Invalid value type");
  }
}

function kvValueToRawValue(kv: KvValue): RawValue {
  switch (kv.kind) {
    case "v8":
      return { kind: "v8", value: kv.data };
    case "bytes":
      return { kind: "bytes", value: kv.data };
    case "u64":
      return { kind: "u64", value: kv.value };
  }
}

// ---------------------------------------------------------------------------
// Hex encoding for versionstamps
// ---------------------------------------------------------------------------

function hexEncode(buf: Uint8Array): string {
  return buf.toHex();
}

function hexDecode(s: string): Uint8Array {
  // deno-lint-ignore prefer-primordials
  return Uint8Array.fromHex(s);
}

// ---------------------------------------------------------------------------
// Remote value conversion (protobuf encoding <-> KvValue)
// ---------------------------------------------------------------------------

function remoteEntryToKvEntry(e: RemoteKvEntry): SqliteKvEntry {
  let value: KvValue;
  switch (e.encoding) {
    case ValueEncoding.VE_V8:
      value = { kind: "v8", data: e.value };
      break;
    case ValueEncoding.VE_LE64:
      value = {
        kind: "u64",
        value: DataViewPrototypeGetBigUint64(
          new DataView(
            TypedArrayPrototypeGetBuffer(e.value),
            TypedArrayPrototypeGetByteOffset(e.value),
            TypedArrayPrototypeGetByteLength(e.value),
          ),
          0,
          true,
        ),
      };
      break;
    case ValueEncoding.VE_BYTES:
      value = { kind: "bytes", data: e.value };
      break;
    default:
      value = { kind: "v8", data: e.value };
  }
  return { key: e.key, value, versionstamp: e.versionstamp };
}

function kvValueToProto(
  value: KvValue,
): { data: Uint8Array; encoding: number } {
  switch (value.kind) {
    case "v8":
      return { data: value.data, encoding: ValueEncoding.VE_V8 };
    case "bytes":
      return { data: value.data, encoding: ValueEncoding.VE_BYTES };
    case "u64": {
      const buf = new Uint8Array(8);
      DataViewPrototypeSetBigUint64(
        new DataView(TypedArrayPrototypeGetBuffer(buf)),
        0,
        value.value,
        true,
      );
      return { data: buf, encoding: ValueEncoding.VE_LE64 };
    }
  }
}

// ---------------------------------------------------------------------------
// Key encoding helpers
// ---------------------------------------------------------------------------

// Config limits (matching the Rust KvConfig defaults)
const MAX_WRITE_KEY_SIZE = 2048;
const MAX_READ_KEY_SIZE = 2049;
const MAX_VALUE_SIZE = 65536;
const MAX_CHECKS = 100;
const MAX_MUTATIONS = 1000;
const MAX_READ_RANGES = 10;
const MAX_READ_ENTRIES = 1000;
const MAX_TOTAL_MUTATION_SIZE = 819200;
const MAX_TOTAL_KEY_SIZE = 81920;

function encodeKvKey(key: Deno.KvKey): Uint8Array {
  if (!ArrayIsArray(key)) {
    throw new TypeError("key must be an array");
  }
  const encoded = encodeKey(
    kvKeyToKeyParts(key as (string | number | bigint | Uint8Array | boolean)[]),
  );
  return encoded;
}

function validateWriteKey(encoded: Uint8Array): void {
  if (TypedArrayPrototypeGetLength(encoded) === 0) {
    throw new TypeError("key cannot be empty");
  }
  if (TypedArrayPrototypeGetLength(encoded) > MAX_WRITE_KEY_SIZE) {
    throw new TypeError(
      `Key too large for write (max ${MAX_WRITE_KEY_SIZE} bytes)`,
    );
  }
}

function validateReadKey(encoded: Uint8Array): void {
  if (TypedArrayPrototypeGetLength(encoded) > MAX_READ_KEY_SIZE) {
    throw new TypeError(
      `Key too large for read (max ${MAX_READ_KEY_SIZE} bytes)`,
    );
  }
}

function validateValue(raw: RawValue): void {
  let size: number;
  switch (raw.kind) {
    case "v8":
      size = TypedArrayPrototypeGetLength(raw.value as Uint8Array);
      break;
    case "bytes":
      size = TypedArrayPrototypeGetLength(raw.value as Uint8Array);
      break;
    case "u64":
      size = 8;
      break;
  }
  if (size > MAX_VALUE_SIZE) {
    throw new TypeError(
      `Value too large (max ${MAX_VALUE_SIZE} bytes)`,
    );
  }
}

function decodeKvKeyBytes(bytes: Uint8Array): Deno.KvKey {
  return keyPartsToKvKey(decodeKey(bytes)) as Deno.KvKey;
}

// ---------------------------------------------------------------------------
// Selector / cursor logic (ported from Rust RawSelector)
// ---------------------------------------------------------------------------

interface RawSelector {
  kind: "prefixed" | "range";
  prefix?: Uint8Array;
  start?: Uint8Array;
  end?: Uint8Array;
}

function selectorFromKv(
  prefix: Deno.KvKey | null,
  start: Deno.KvKey | null,
  end: Deno.KvKey | null,
): RawSelector {
  const prefixBytes = prefix ? encodeKvKey(prefix) : null;
  const startBytes = start ? encodeKvKey(start) : null;
  const endBytes = end ? encodeKvKey(end) : null;

  if (prefixBytes && !startBytes && !endBytes) {
    return { kind: "prefixed", prefix: prefixBytes };
  }
  if (prefixBytes && startBytes && !endBytes) {
    if (
      !bytesStartsWith(startBytes, prefixBytes) ||
      TypedArrayPrototypeGetLength(startBytes) ===
        TypedArrayPrototypeGetLength(prefixBytes)
    ) {
      throw new TypeError("Start key is not in the keyspace defined by prefix");
    }
    return { kind: "prefixed", prefix: prefixBytes, start: startBytes };
  }
  if (prefixBytes && !startBytes && endBytes) {
    if (
      !bytesStartsWith(endBytes, prefixBytes) ||
      TypedArrayPrototypeGetLength(endBytes) ===
        TypedArrayPrototypeGetLength(prefixBytes)
    ) {
      throw new TypeError("End key is not in the keyspace defined by prefix");
    }
    return { kind: "prefixed", prefix: prefixBytes, end: endBytes };
  }
  if (!prefixBytes && startBytes && endBytes) {
    if (compareBytes(startBytes, endBytes) > 0) {
      throw new TypeError("Start key is greater than end key");
    }
    return { kind: "range", start: startBytes, end: endBytes };
  }
  if (!prefixBytes && startBytes && !endBytes) {
    const startLen = TypedArrayPrototypeGetLength(startBytes);
    const endComputed = new Uint8Array(startLen + 1);
    TypedArrayPrototypeSet(endComputed, startBytes);
    endComputed[startLen] = 0;
    return { kind: "range", start: startBytes, end: endComputed };
  }
  throw new TypeError(
    "Selector must specify either 'prefix' or both 'start' and 'end' key",
  );
}

function selectorRangeStart(sel: RawSelector): Uint8Array {
  if (sel.start) return sel.start;
  if (sel.prefix) {
    const prefixLen = TypedArrayPrototypeGetLength(sel.prefix);
    const r = new Uint8Array(prefixLen + 1);
    TypedArrayPrototypeSet(r, sel.prefix);
    r[prefixLen] = 0x00;
    return r;
  }
  throw new TypeError("Invalid selector");
}

function selectorRangeEnd(sel: RawSelector): Uint8Array {
  if (sel.end) return sel.end;
  if (sel.prefix) {
    const prefixLen = TypedArrayPrototypeGetLength(sel.prefix);
    const r = new Uint8Array(prefixLen + 1);
    TypedArrayPrototypeSet(r, sel.prefix);
    r[prefixLen] = 0xff;
    return r;
  }
  throw new TypeError("Invalid selector");
}

function selectorCommonPrefix(sel: RawSelector): Uint8Array {
  if (sel.prefix) return sel.prefix;
  const start = sel.start!;
  const end = sel.end!;
  const startLen = TypedArrayPrototypeGetLength(start);
  const endLen = TypedArrayPrototypeGetLength(end);
  let i = 0;
  while (i < startLen && i < endLen && start[i] === end[i]) i++;
  return TypedArrayPrototypeSlice(start, 0, i);
}

function encodeCursorFromSelector(
  sel: RawSelector,
  boundaryKey: Uint8Array,
): string {
  const common = selectorCommonPrefix(sel);
  if (!bytesStartsWith(boundaryKey, common)) {
    throw new TypeError("Invalid boundary key");
  }
  return base64urlEncode(
    TypedArrayPrototypeSlice(
      boundaryKey,
      TypedArrayPrototypeGetLength(common),
    ),
  );
}

function decodeSelectorAndCursor(
  sel: RawSelector,
  reverse: boolean,
  cursor: string | undefined,
): { start: Uint8Array; end: Uint8Array } {
  if (!cursor) {
    return { start: selectorRangeStart(sel), end: selectorRangeEnd(sel) };
  }

  const common = selectorCommonPrefix(sel);
  const cursorBytes = base64urlDecode(cursor);
  const commonLen = TypedArrayPrototypeGetLength(common);
  const cursorLen = TypedArrayPrototypeGetLength(cursorBytes);

  let firstKey: Uint8Array;
  let lastKey: Uint8Array;

  if (reverse) {
    firstKey = selectorRangeStart(sel);
    lastKey = new Uint8Array(commonLen + cursorLen);
    TypedArrayPrototypeSet(lastKey, common);
    TypedArrayPrototypeSet(lastKey, cursorBytes, commonLen);
  } else {
    firstKey = new Uint8Array(commonLen + cursorLen + 1);
    TypedArrayPrototypeSet(firstKey, common);
    TypedArrayPrototypeSet(firstKey, cursorBytes, commonLen);
    firstKey[commonLen + cursorLen] = 0;
    lastKey = selectorRangeEnd(sel);
  }

  return { start: firstKey, end: lastKey };
}

// ---------------------------------------------------------------------------
// Byte comparison helpers
// ---------------------------------------------------------------------------

function bytesStartsWith(a: Uint8Array, prefix: Uint8Array): boolean {
  const aLen = TypedArrayPrototypeGetLength(a);
  const prefixLen = TypedArrayPrototypeGetLength(prefix);
  if (aLen < prefixLen) return false;
  for (let i = 0; i < prefixLen; i++) {
    if (a[i] !== prefix[i]) return false;
  }
  return true;
}

function compareBytes(a: Uint8Array, b: Uint8Array): number {
  const aLen = TypedArrayPrototypeGetLength(a);
  const bLen = TypedArrayPrototypeGetLength(b);
  const len = MathMin(aLen, bLen);
  for (let i = 0; i < len; i++) {
    if (a[i] < b[i]) return -1;
    if (a[i] > b[i]) return 1;
  }
  return aLen - bLen;
}

// ---------------------------------------------------------------------------
// Backend abstraction
// ---------------------------------------------------------------------------

interface KvBackend {
  snapshotRead(
    ranges: ReadRange[],
    consistency: "strong" | "eventual",
  ): Promise<SqliteKvEntry[][]>;

  atomicWrite(
    checks: SqliteCheck[],
    mutations: SqliteMutation[],
    enqueues: SqliteEnqueue[],
  ): Promise<CommitResult | null>;

  dequeueNextMessage(): Promise<{ payload: Uint8Array; id: string } | null>;
  finishMessage(id: string, success: boolean): Promise<void>;

  watch(keys: Uint8Array[]): ReadableStream;

  close(): void;
}

// Wrap SqliteBackend into async interface
class SqliteKvBackend implements KvBackend {
  #backend: SqliteBackend;

  constructor(path: string) {
    this.#backend = new SqliteBackend(path);
  }

  snapshotRead(
    ranges: ReadRange[],
    _consistency: "strong" | "eventual",
  ): Promise<SqliteKvEntry[][]> {
    return PromiseResolve(this.#backend.snapshotRead(ranges));
  }

  atomicWrite(
    checks: SqliteCheck[],
    mutations: SqliteMutation[],
    enqueues: SqliteEnqueue[],
  ): Promise<CommitResult | null> {
    return PromiseResolve(
      this.#backend.atomicWrite(checks, mutations, enqueues),
    );
  }

  dequeueNextMessage(): Promise<{ payload: Uint8Array; id: string } | null> {
    return PromiseResolve(this.#backend.dequeueNextMessage());
  }

  finishMessage(id: string, success: boolean): Promise<void> {
    this.#backend.finishMessage(id, success);
    return PromiseResolve();
  }

  watch(keys: Uint8Array[]): ReadableStream {
    // SQLite watch: poll-based implementation
    // For now, return a stream that polls every 500ms
    const backend = this.#backend;
    const lastVersionstamps: (string | null)[] = ArrayPrototypeMap(
      keys,
      () => null,
    );
    let closed = false;

    return new ReadableStream({
      async pull(controller) {
        while (!closed) {
          const ranges: ReadRange[] = [];
          for (let i = 0; i < keys.length; i++) {
            const k = keys[i];
            const kLen = TypedArrayPrototypeGetLength(k);
            const endKey = new Uint8Array(kLen + 1);
            TypedArrayPrototypeSet(endKey, k);
            endKey[kLen] = 0;
            ArrayPrototypePush(ranges, {
              start: k,
              end: endKey,
              limit: 1,
              reverse: false,
            });
          }

          const results = backend.snapshotRead(ranges);
          let changed = false;
          const entries: (Deno.KvEntryMaybe<unknown>)[] = [];

          for (let i = 0; i < keys.length; i++) {
            const entry = results[i][0];
            const vs = entry ? hexEncode(entry.versionstamp) : null;
            if (vs !== lastVersionstamps[i]) {
              changed = true;
              lastVersionstamps[i] = vs;
            }
            if (entry) {
              const decodedKey = decodeKvKeyBytes(entry.key);
              ArrayPrototypePush(entries, {
                key: decodedKey,
                value: deserializeRawValue(kvValueToRawValue(entry.value)),
                versionstamp: hexEncode(entry.versionstamp),
              });
            } else {
              ArrayPrototypePush(entries, {
                key: decodeKvKeyBytes(keys[i]),
                value: null,
                versionstamp: null,
              });
            }
          }

          if (changed) {
            controller.enqueue(entries);
            return;
          }

          // Poll interval
          await new Promise((r) => setTimeout(r, 500));
        }
      },
      cancel() {
        closed = true;
      },
    });
  }

  close() {
    this.#backend.close();
  }
}

// Wrap RemoteBackend into the KvBackend interface
class RemoteKvBackend implements KvBackend {
  #backend: RemoteBackend;

  constructor(url: string, accessToken: string) {
    this.#backend = new RemoteBackend(url, accessToken);
  }

  async snapshotRead(
    ranges: ReadRange[],
    consistency: "strong" | "eventual",
  ): Promise<SqliteKvEntry[][]> {
    const protoRanges: ProtoReadRange[] = ArrayPrototypeMap(
      ranges,
      (r: ReadRange) => ({
        start: r.start,
        end: r.end,
        limit: r.limit,
        reverse: r.reverse,
      }),
    );
    const result = await this.#backend.snapshotRead(protoRanges, consistency);
    return ArrayPrototypeMap(
      result,
      (entries: RemoteKvEntry[]) =>
        ArrayPrototypeMap(entries, remoteEntryToKvEntry),
    );
  }

  async atomicWrite(
    checks: SqliteCheck[],
    mutations: SqliteMutation[],
    enqueues: SqliteEnqueue[],
  ): Promise<CommitResult | null> {
    const protoChecks: ProtoCheck[] = ArrayPrototypeMap(
      checks,
      (c: SqliteCheck) => ({
        key: c.key,
        versionstamp: c.versionstamp ?? new Uint8Array(0),
      }),
    );

    const protoMutations: ProtoMutation[] = ArrayPrototypeMap(
      mutations,
      (m: SqliteMutation) => {
        let mutationType: number;
        let value: { data: Uint8Array; encoding: number } | null = null;
        let sumMin = new Uint8Array(0);
        let sumMax = new Uint8Array(0);
        let sumClamp = false;

        switch (m.kind.type) {
          case "set":
            mutationType = MutationType.M_SET;
            value = kvValueToProto(m.kind.value);
            break;
          case "delete":
            mutationType = MutationType.M_DELETE;
            break;
          case "sum":
            mutationType = MutationType.M_SUM;
            value = kvValueToProto(m.kind.value);
            sumMin = m.kind.minV8;
            sumMax = m.kind.maxV8;
            sumClamp = m.kind.clamp;
            break;
          case "min":
            mutationType = MutationType.M_MIN;
            value = kvValueToProto(m.kind.value);
            break;
          case "max":
            mutationType = MutationType.M_MAX;
            value = kvValueToProto(m.kind.value);
            break;
          case "setSuffixVersionstampedKey":
            mutationType = MutationType.M_SET_SUFFIX_VERSIONSTAMPED_KEY;
            value = kvValueToProto(m.kind.value);
            break;
          default:
            throw new TypeError("Unknown mutation type");
        }

        return {
          key: m.key,
          value: value
            ? {
              data: value.data,
              encoding: value.encoding as ValueEncoding,
            }
            : null,
          mutationType: mutationType as MutationType,
          expireAtMs: m.expireAt ? BigInt(m.expireAt) : 0n,
          sumMin,
          sumMax,
          sumClamp,
        };
      },
    );

    const protoEnqueues: ProtoEnqueue[] = ArrayPrototypeMap(
      enqueues,
      (e: SqliteEnqueue) => ({
        payload: e.payload,
        deadlineMs: BigInt(e.deadlineMs),
        keysIfUndelivered: e.keysIfUndelivered,
        backoffSchedule: e.backoffSchedule ?? [],
      }),
    );

    const result = await this.#backend.atomicWrite(
      protoChecks,
      protoMutations,
      protoEnqueues,
    );
    if (!result) return null;
    return { versionstamp: result.versionstamp };
  }

  dequeueNextMessage(): Promise<{ payload: Uint8Array; id: string } | null> {
    // Queue operations not supported on remote backend
    return PromiseResolve(null);
  }

  finishMessage(_id: string, _success: boolean): Promise<void> {
    // Queue operations not supported on remote backend
    return PromiseResolve();
  }

  watch(keys: Uint8Array[]): ReadableStream {
    const raw = this.#backend.watch(keys);
    // Transform WatchKeyUpdate[] to the format expected by the Kv.watch() API
    return raw;
  }

  close() {
    this.#backend.close();
  }
}

// ---------------------------------------------------------------------------
// KvU64
// ---------------------------------------------------------------------------

const MIN_U64 = 0n;
const MAX_U64 = 0xFFFFFFFFFFFFFFFFn;

class KvU64 {
  value: bigint;

  constructor(value: bigint) {
    if (typeof value !== "bigint") {
      throw new TypeError(`Value must be a bigint: received ${typeof value}`);
    }
    if (value < MIN_U64) {
      throw new RangeError(
        `Value must be a positive bigint: received ${value}`,
      );
    }
    if (value > MAX_U64) {
      throw new RangeError("Value must fit in a 64-bit unsigned integer");
    }
    this.value = value;
    ObjectFreeze(this);
  }

  valueOf(): bigint {
    return this.value;
  }

  toString(): string {
    return BigIntPrototypeToString(this.value);
  }

  get [SymbolToStringTag](): string {
    return "Deno.KvU64";
  }

  [SymbolFor("Deno.privateCustomInspect")](
    inspect: (v: unknown, opts?: unknown) => string,
    inspectOptions: unknown,
  ): string {
    return StringPrototypeReplace(
      inspect(Object(this.value), inspectOptions),
      "BigInt",
      "Deno.KvU64",
    );
  }
}

const KvU64Prototype = KvU64.prototype;

// ---------------------------------------------------------------------------
// Config limits (matching the Rust KvConfig defaults)
// ---------------------------------------------------------------------------

const MAX_QUEUE_DELAY = 30 * 24 * 60 * 60 * 1000;
const MAX_QUEUE_BACKOFF_INTERVALS = 5;
const MAX_QUEUE_BACKOFF_INTERVAL = 60 * 60 * 1000;

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

function validateQueueDelay(delay: number) {
  if (delay < 0) throw new TypeError(`Delay must be >= 0: received ${delay}`);
  if (delay > MAX_QUEUE_DELAY) {
    throw new TypeError(
      `Delay cannot be greater than 30 days: received ${delay}`,
    );
  }
  if (NumberIsNaN(delay)) throw new TypeError("Delay cannot be NaN");
}

function validateBackoffSchedule(schedule: number[]) {
  if (schedule.length > MAX_QUEUE_BACKOFF_INTERVALS) {
    throw new TypeError(
      `Invalid backoffSchedule, max ${MAX_QUEUE_BACKOFF_INTERVALS} intervals allowed`,
    );
  }
  for (let i = 0; i < schedule.length; i++) {
    if (
      schedule[i] < 0 ||
      schedule[i] > MAX_QUEUE_BACKOFF_INTERVAL ||
      NumberIsNaN(schedule[i])
    ) {
      throw new TypeError(
        `Invalid backoffSchedule, interval at index ${i} is invalid`,
      );
    }
  }
}

// ---------------------------------------------------------------------------
// Kv class
// ---------------------------------------------------------------------------

const kvSymbol = Symbol("Kv");
const commitVersionstampSymbol = Symbol("KvCommitVersionstamp");

class Kv {
  #backend: KvBackend;
  #isClosed = false;
  #closeResolve: (() => void) | null = null;
  #closePromise: Promise<void>;

  constructor(backend: KvBackend, symbol: symbol) {
    if (kvSymbol !== symbol) {
      throw new TypeError(
        "Deno.Kv can not be constructed: use Deno.openKv instead",
      );
    }
    this.#backend = backend;
    this.#closePromise = new Promise((resolve) => {
      this.#closeResolve = resolve;
    });
  }

  atomic(): AtomicOperation {
    return new AtomicOperation(this.#backend);
  }

  commitVersionstamp(): symbol {
    return commitVersionstampSymbol;
  }

  async get(
    key: Deno.KvKey,
    opts?: { consistency?: Deno.KvConsistencyLevel },
  ): Promise<Deno.KvEntryMaybe<unknown>> {
    const encodedKey = encodeKvKey(key);
    validateReadKey(encodedKey);
    const encodedKeyLen = TypedArrayPrototypeGetLength(encodedKey);
    const endKey = new Uint8Array(encodedKeyLen + 1);
    TypedArrayPrototypeSet(endKey, encodedKey);
    endKey[encodedKeyLen] = 0;
    validateReadKey(endKey);

    const readResult = await this.#backend.snapshotRead(
      [{ start: encodedKey, end: endKey, limit: 1, reverse: false }],
      opts?.consistency ?? "strong",
    );
    const entries = readResult[0];

    if (!entries.length) {
      return { key, value: null, versionstamp: null };
    }

    const e = entries[0];
    return {
      key: decodeKvKeyBytes(e.key),
      value: deserializeRawValue(kvValueToRawValue(e.value)),
      versionstamp: hexEncode(e.versionstamp),
    };
  }

  async getMany(
    keys: Deno.KvKey[],
    opts?: { consistency?: Deno.KvConsistencyLevel },
  ): Promise<Deno.KvEntryMaybe<unknown>[]> {
    if (keys.length > MAX_READ_RANGES) {
      throw new TypeError(`Too many ranges (max ${MAX_READ_RANGES})`);
    }
    const ranges: ReadRange[] = ArrayPrototypeMap(
      keys,
      (key: Deno.KvKey) => {
        const encodedKey = encodeKvKey(key);
        validateReadKey(encodedKey);
        const encodedKeyLen = TypedArrayPrototypeGetLength(encodedKey);
        const endKey = new Uint8Array(encodedKeyLen + 1);
        TypedArrayPrototypeSet(endKey, encodedKey);
        endKey[encodedKeyLen] = 0;
        return { start: encodedKey, end: endKey, limit: 1, reverse: false };
      },
    );

    const results = await this.#backend.snapshotRead(
      ranges,
      opts?.consistency ?? "strong",
    );

    return ArrayPrototypeMap(
      results,
      (entries: SqliteKvEntry[], i: number) => {
        if (!entries.length) {
          return { key: keys[i], value: null, versionstamp: null };
        }
        const e = entries[0];
        return {
          key: decodeKvKeyBytes(e.key),
          value: deserializeRawValue(kvValueToRawValue(e.value)),
          versionstamp: hexEncode(e.versionstamp),
        };
      },
    );
  }

  async set(
    key: Deno.KvKey,
    value: unknown,
    options?: { expireIn?: number },
  ): Promise<Deno.KvCommitResult> {
    const encodedKey = encodeKvKey(key);
    validateWriteKey(encodedKey);
    const raw = serializeValue(value);
    validateValue(raw);
    const result = await doAtomicWrite(
      this.#backend,
      [],
      [
        {
          key: encodedKey,
          kind: {
            type: "set",
            value: rawValueToKvValue(raw),
          },
          expireAt: options?.expireIn ? DateNow() + options.expireIn : null,
        },
      ],
      [],
    );
    if (!result) throw new TypeError("Failed to set value");
    return { ok: true, versionstamp: hexEncode(result.versionstamp) };
  }

  async delete(key: Deno.KvKey): Promise<void> {
    const result = await doAtomicWrite(
      this.#backend,
      [],
      [{ key: encodeKvKey(key), kind: { type: "delete" }, expireAt: null }],
      [],
    );
    if (!result) throw new TypeError("Failed to delete value");
  }

  list(
    selector: Deno.KvListSelector,
    options: {
      limit?: number;
      batchSize?: number;
      cursor?: string;
      reverse?: boolean;
      consistency?: Deno.KvConsistencyLevel;
    } = { __proto__: null } as {
      limit?: number;
      batchSize?: number;
      cursor?: string;
      reverse?: boolean;
      consistency?: Deno.KvConsistencyLevel;
    },
  ): KvListIterator {
    if (options.limit !== undefined && options.limit <= 0) {
      throw new Error(`Limit must be positive: received ${options.limit}`);
    }

    let batchSize = options.batchSize ?? (options.limit ?? 100);
    if (batchSize <= 0) throw new Error("batchSize must be positive");
    if (options.batchSize === undefined && batchSize > 500) batchSize = 500;
    if (batchSize > MAX_READ_ENTRIES) {
      throw new TypeError(`Too many entries (max ${MAX_READ_ENTRIES})`);
    }

    const backend = this.#backend;
    const pullBatch = async (
      sel: Deno.KvListSelector,
      cursor: string | undefined,
      reverse: boolean,
      consistency: Deno.KvConsistencyLevel,
    ): Promise<Deno.KvEntry<unknown>[]> => {
      const prefix = ReflectHas(sel, "prefix")
        ? (sel as { prefix: Deno.KvKey }).prefix
        : null;
      const start = ReflectHas(sel, "start")
        ? (sel as { start: Deno.KvKey }).start
        : null;
      const end = ReflectHas(sel, "end")
        ? (sel as { end: Deno.KvKey }).end
        : null;

      const rawSel = selectorFromKv(
        prefix ?? null,
        start ?? null,
        end ?? null,
      );
      const { start: rangeStart, end: rangeEnd } = decodeSelectorAndCursor(
        rawSel,
        reverse,
        cursor,
      );

      const readResult = await backend.snapshotRead(
        [{ start: rangeStart, end: rangeEnd, limit: batchSize, reverse }],
        consistency,
      );
      const entries = readResult[0];

      return ArrayPrototypeMap(
        entries,
        (e: SqliteKvEntry) => ({
          key: decodeKvKeyBytes(e.key),
          value: deserializeRawValue(kvValueToRawValue(e.value)),
          versionstamp: hexEncode(e.versionstamp),
        }),
      );
    };

    return new KvListIterator({
      limit: options.limit,
      selector,
      cursor: options.cursor,
      reverse: options.reverse ?? false,
      consistency: options.consistency ?? "strong",
      batchSize,
      pullBatch,
    });
  }

  async enqueue(
    message: unknown,
    opts?: {
      delay?: number;
      keysIfUndelivered?: Deno.KvKey[];
      backoffSchedule?: number[];
    },
  ): Promise<Deno.KvCommitResult> {
    if (opts?.delay !== undefined) validateQueueDelay(opts.delay);
    if (opts?.backoffSchedule !== undefined) {
      validateBackoffSchedule(opts.backoffSchedule);
    }

    const result = await doAtomicWrite(this.#backend, [], [], [
      {
        payload: core.serialize(message, { forStorage: true }),
        deadlineMs: DateNow() + (opts?.delay ?? 0),
        keysIfUndelivered: ArrayPrototypeMap(
          opts?.keysIfUndelivered ?? [],
          encodeKvKey,
        ),
        backoffSchedule: opts?.backoffSchedule ?? null,
      },
    ]);
    if (!result) throw new TypeError("Failed to enqueue value");
    return { ok: true, versionstamp: hexEncode(result.versionstamp) };
  }

  async listenQueue(
    handler: (message: unknown) => Promise<void> | void,
  ): Promise<void> {
    if (this.#isClosed) throw new Error("Queue already closed");

    const closedSentinel = Symbol("closed");
    const inflightFinishes: Promise<void>[] = [];

    while (!this.#isClosed) {
      let next: { payload: Uint8Array; id: string } | null;
      try {
        next = await this.#backend.dequeueNextMessage();
      } catch (e) {
        // DB may have been closed
        if (this.#isClosed) break;
        throw e;
      }
      if (next === null) {
        if (this.#isClosed) break;
        // Poll interval when no messages - race against close.
        // We must clear the timer if close wins to avoid leaking an async op
        // that would trip the test sanitizer.
        let timerId: number | undefined;
        const winner = await SafePromiseRace([
          new Promise((r) => {
            timerId = setTimeout(r, 100);
          }),
          PromisePrototypeThen(this.#closePromise, () => closedSentinel),
        ]);
        if (winner === closedSentinel) {
          clearTimeout(timerId);
          break;
        }
        continue;
      }

      const deserialized = core.deserialize(next.payload, {
        forStorage: true,
      });

      // Dispatch handler concurrently (IIFE), matching the old Rust-backed
      // JS implementation. The dequeue loop continues immediately so
      // multiple handlers can run in parallel.
      const messageId = next.id;
      const backend = this.#backend;
      const finishPromise = (async () => {
        let success = false;
        try {
          const result = handler(deserialized);
          if (
            result && typeof (result as Promise<void>).then === "function"
          ) {
            await (result as Promise<void>);
          }
          success = true;
        } catch (error) {
          // deno-lint-ignore no-console
          console.error("Exception in queue handler", error);
        }
        try {
          await backend.finishMessage(messageId, success);
        } catch {
          // DB may have been closed
        }
      })();
      ArrayPrototypePush(inflightFinishes, finishPromise);
    }

    // Wait for in-flight handlers to finish or close to resolve
    if (inflightFinishes.length > 0) {
      await SafePromiseRace([
        // deno-lint-ignore prefer-primordials
        Promise.allSettled(inflightFinishes),
        this.#closePromise,
      ]);
    }
  }

  watch(
    keys: Deno.KvKey[],
    options: { raw?: boolean } = { __proto__: null } as { raw?: boolean },
  ): ReadableStream<Deno.KvEntryMaybe<unknown>[]> {
    const encodedKeys = ArrayPrototypeMap(keys, encodeKvKey);
    const raw = options.raw ?? false;
    const rawStream = this.#backend.watch(encodedKeys);

    // For SQLite backend, the stream already produces decoded entries
    // For remote, we need to transform the protobuf entries
    // The Kv.watch() method in the original code does deduplication
    // based on versionstamp changes - replicate that here.
    const lastEntries: (Deno.KvEntryMaybe<unknown> | undefined)[] = new Array(
      keys.length,
    );

    const reader = rawStream.getReader();

    return new ReadableStream({
      async pull(controller) {
        while (true) {
          const { done, value: updates } = await reader.read();
          if (done) {
            controller.close();
            return;
          }

          let changed = false;
          for (let i = 0; i < keys.length; i++) {
            const update = (updates as Record<
              number,
              Deno.KvEntryMaybe<unknown> | "unchanged" | null
            >)[i];
            if (!update || update === "unchanged") {
              if (lastEntries[i] === undefined) {
                throw new Error(
                  "'watch': invalid unchanged update (internal error)",
                );
              }
              continue;
            }
            const vs = update?.versionstamp ?? null;
            if (
              lastEntries[i] !== undefined &&
              vs === lastEntries[i]?.versionstamp
            ) {
              continue;
            }
            changed = true;
            if (update.value === null) {
              lastEntries[i] = {
                key: ArrayPrototypeSlice(keys[i], 0),
                value: null,
                versionstamp: null,
              };
            } else {
              lastEntries[i] = update;
            }
          }
          if (!changed && !raw) continue;
          controller.enqueue(
            ArrayPrototypeMap(
              lastEntries,
              (e: Deno.KvEntryMaybe<unknown> | undefined) =>
                ObjectAssign({}, e!),
            ),
          );
          return;
        }
      },
      cancel() {
        reader.cancel();
      },
    });
  }

  close(): void {
    this.#backend.close();
    this.#isClosed = true;
    if (this.#closeResolve) {
      this.#closeResolve();
      this.#closeResolve = null;
    }
  }

  [SymbolDispose](): void {
    try {
      this.close();
    } catch {
      // already closed
    }
  }
}

// ---------------------------------------------------------------------------
// AtomicOperation
// ---------------------------------------------------------------------------

class AtomicOperation {
  #backend: KvBackend;
  #checks: SqliteCheck[] = [];
  #mutations: SqliteMutation[] = [];
  #enqueues: SqliteEnqueue[] = [];

  constructor(backend: KvBackend) {
    this.#backend = backend;
  }

  check(...checks: Deno.AtomicCheck[]): this {
    for (let ci = 0; ci < checks.length; ci++) {
      const check = checks[ci];
      const encodedKey = encodeKvKey(check.key);
      validateWriteKey(encodedKey);
      let versionstamp: Uint8Array | null = null;
      if (check.versionstamp !== null && check.versionstamp !== undefined) {
        if (
          typeof check.versionstamp !== "string" ||
          check.versionstamp.length !== 20 ||
          !RegExpPrototypeTest(versionstampRe, check.versionstamp)
        ) {
          throw new TypeError("invalid versionstamp");
        }
        versionstamp = hexDecode(check.versionstamp);
      }
      ArrayPrototypePush(this.#checks, { key: encodedKey, versionstamp });
    }
    return this;
  }

  mutate(...mutations: Deno.KvMutation[]): this {
    for (let mi = 0; mi < mutations.length; mi++) {
      const m = mutations[mi];
      const key = encodeKvKey(m.key);
      validateWriteKey(key);
      let kind: MutationKind;

      switch (m.type) {
        case "delete":
          if (m.value) {
            throw new TypeError("Invalid mutation 'delete' with value");
          }
          kind = { type: "delete" };
          break;
        case "set":
          if (!ReflectHas(m, "value")) {
            throw new TypeError("Invalid mutation 'set' without value");
          }
          kind = {
            type: "set",
            value: rawValueToKvValue(serializeValue(m.value)),
          };
          break;
        case "sum":
          if (!ReflectHas(m, "value")) {
            throw new TypeError("Invalid mutation 'sum' without value");
          }
          kind = {
            type: "sum",
            value: rawValueToKvValue(serializeValue(m.value)),
            minV8: new Uint8Array(0),
            maxV8: new Uint8Array(0),
            clamp: false,
          };
          break;
        case "min":
          if (!ReflectHas(m, "value")) {
            throw new TypeError("Invalid mutation 'min' without value");
          }
          kind = {
            type: "min",
            value: rawValueToKvValue(serializeValue(m.value)),
          };
          break;
        case "max":
          if (!ReflectHas(m, "value")) {
            throw new TypeError("Invalid mutation 'max' without value");
          }
          kind = {
            type: "max",
            value: rawValueToKvValue(serializeValue(m.value)),
          };
          break;
        default:
          throw new TypeError("Invalid mutation type");
      }

      ArrayPrototypePush(this.#mutations, {
        key,
        kind,
        expireAt: (ReflectHas(m, "expireIn") && typeof m.expireIn === "number")
          ? DateNow() + m.expireIn
          : null,
      });
    }
    return this;
  }

  sum(key: Deno.KvKey, n: bigint): this {
    ArrayPrototypePush(this.#mutations, {
      key: encodeKvKey(key),
      kind: {
        type: "sum",
        value: { kind: "u64", value: new KvU64(n).value },
        minV8: new Uint8Array(0),
        maxV8: new Uint8Array(0),
        clamp: false,
      },
      expireAt: null,
    });
    return this;
  }

  min(key: Deno.KvKey, n: bigint): this {
    ArrayPrototypePush(this.#mutations, {
      key: encodeKvKey(key),
      kind: { type: "min", value: { kind: "u64", value: new KvU64(n).value } },
      expireAt: null,
    });
    return this;
  }

  max(key: Deno.KvKey, n: bigint): this {
    ArrayPrototypePush(this.#mutations, {
      key: encodeKvKey(key),
      kind: { type: "max", value: { kind: "u64", value: new KvU64(n).value } },
      expireAt: null,
    });
    return this;
  }

  set(key: Deno.KvKey, value: unknown, options?: { expireIn?: number }): this {
    let actualKey = key;
    let mutationType: "set" | "setSuffixVersionstampedKey" = "set";

    // Handle commitVersionstamp symbol as last key element
    if (
      key.length > 0 &&
      key[key.length - 1] === commitVersionstampSymbol
    ) {
      actualKey = ArrayPrototypeSlice(key, 0, -1);
      mutationType = "setSuffixVersionstampedKey";
    }

    ArrayPrototypePush(this.#mutations, {
      key: encodeKvKey(actualKey),
      kind: {
        type: mutationType,
        value: rawValueToKvValue(serializeValue(value)),
      },
      expireAt: options?.expireIn ? DateNow() + options.expireIn : null,
    });
    return this;
  }

  delete(key: Deno.KvKey): this {
    ArrayPrototypePush(this.#mutations, {
      key: encodeKvKey(key),
      kind: { type: "delete" },
      expireAt: null,
    });
    return this;
  }

  enqueue(
    message: unknown,
    opts?: {
      delay?: number;
      keysIfUndelivered?: Deno.KvKey[];
      backoffSchedule?: number[];
    },
  ): this {
    if (opts?.delay !== undefined) validateQueueDelay(opts.delay);
    if (opts?.backoffSchedule !== undefined) {
      validateBackoffSchedule(opts.backoffSchedule);
    }
    ArrayPrototypePush(this.#enqueues, {
      payload: core.serialize(message, { forStorage: true }),
      deadlineMs: DateNow() + (opts?.delay ?? 0),
      keysIfUndelivered: ArrayPrototypeMap(
        opts?.keysIfUndelivered ?? [],
        encodeKvKey,
      ),
      backoffSchedule: opts?.backoffSchedule ?? null,
    });
    return this;
  }

  async commit(): Promise<Deno.KvCommitResult | Deno.KvCommitError> {
    if (this.#checks.length > MAX_CHECKS) {
      throw new TypeError(`Too many checks (max ${MAX_CHECKS})`);
    }
    if (this.#mutations.length + this.#enqueues.length > MAX_MUTATIONS) {
      throw new TypeError(`Too many mutations (max ${MAX_MUTATIONS})`);
    }

    // Validate total mutation and key sizes
    let totalMutationSize = 0;
    let totalKeySize = 0;
    for (let i = 0; i < this.#mutations.length; i++) {
      const m = this.#mutations[i];
      const keyLen = TypedArrayPrototypeGetLength(m.key);
      totalKeySize += keyLen;
      totalMutationSize += keyLen;
      if (m.kind.type !== "delete") {
        const v = m.kind.value;
        const vSize = v.kind === "u64"
          ? 8
          : TypedArrayPrototypeGetLength(v.data);
        totalMutationSize += vSize + keyLen;
      }
    }
    for (let i = 0; i < this.#enqueues.length; i++) {
      const e = this.#enqueues[i];
      const payloadLen = TypedArrayPrototypeGetLength(e.payload);
      totalMutationSize += payloadLen;
      if (payloadLen > MAX_VALUE_SIZE) {
        throw new TypeError(
          `enqueue payload too large (max ${MAX_VALUE_SIZE} bytes)`,
        );
      }
    }
    if (totalMutationSize > MAX_TOTAL_MUTATION_SIZE) {
      throw new TypeError(
        `Total mutation size too large (max ${MAX_TOTAL_MUTATION_SIZE} bytes)`,
      );
    }
    if (totalKeySize > MAX_TOTAL_KEY_SIZE) {
      throw new TypeError(
        `Total key size too large (max ${MAX_TOTAL_KEY_SIZE} bytes)`,
      );
    }

    const result = await doAtomicWrite(
      this.#backend,
      this.#checks,
      this.#mutations,
      this.#enqueues,
    );
    if (!result) return { ok: false };
    return { ok: true, versionstamp: hexEncode(result.versionstamp) };
  }

  then(): never {
    throw new TypeError(
      "'Deno.AtomicOperation' is not a promise: did you forget to call 'commit()'",
    );
  }
}

async function doAtomicWrite(
  backend: KvBackend,
  checks: SqliteCheck[],
  mutations: SqliteMutation[],
  enqueues: SqliteEnqueue[],
): Promise<CommitResult | null> {
  return await backend.atomicWrite(checks, mutations, enqueues);
}

// ---------------------------------------------------------------------------
// KvListIterator
// ---------------------------------------------------------------------------

class KvListIterator extends Object
  implements AsyncIterableIterator<Deno.KvEntry<unknown>> {
  #selector: Deno.KvListSelector;
  #entries: Deno.KvEntry<unknown>[] | null = null;
  #cursorGen: (() => string) | null = null;
  #done = false;
  #lastBatch = false;
  #pullBatch: (
    selector: Deno.KvListSelector,
    cursor: string | undefined,
    reverse: boolean,
    consistency: Deno.KvConsistencyLevel,
  ) => Promise<Deno.KvEntry<unknown>[]>;
  #limit: number | undefined;
  #count = 0;
  #reverse: boolean;
  #batchSize: number;
  #consistency: Deno.KvConsistencyLevel;

  constructor({
    limit,
    selector,
    cursor,
    reverse,
    consistency,
    batchSize,
    pullBatch,
  }: {
    limit?: number;
    selector: Deno.KvListSelector;
    cursor?: string;
    reverse: boolean;
    batchSize: number;
    consistency: Deno.KvConsistencyLevel;
    pullBatch: (
      selector: Deno.KvListSelector,
      cursor: string | undefined,
      reverse: boolean,
      consistency: Deno.KvConsistencyLevel,
    ) => Promise<Deno.KvEntry<unknown>[]>;
  }) {
    super();
    // Validate and freeze selector
    let prefix: Deno.KvKey | undefined;
    let start: Deno.KvKey | undefined;
    let end: Deno.KvKey | undefined;

    if (
      ReflectHas(selector, "prefix") &&
      (selector as { prefix?: Deno.KvKey }).prefix !== undefined
    ) {
      prefix = ObjectFreeze(
        ArrayFrom((selector as { prefix: Deno.KvKey }).prefix),
      );
    }
    if (
      ReflectHas(selector, "start") &&
      (selector as { start?: Deno.KvKey }).start !== undefined
    ) {
      start = ObjectFreeze(
        ArrayFrom((selector as { start: Deno.KvKey }).start),
      );
    }
    if (
      ReflectHas(selector, "end") &&
      (selector as { end?: Deno.KvKey }).end !== undefined
    ) {
      end = ObjectFreeze(
        ArrayFrom((selector as { end: Deno.KvKey }).end),
      );
    }

    if (prefix) {
      if (start && end) {
        throw new TypeError(
          "Selector can not specify both 'start' and 'end' key when specifying 'prefix'",
        );
      }
      if (start) this.#selector = { prefix, start };
      else if (end) this.#selector = { prefix, end };
      else this.#selector = { prefix };
    } else {
      if (start && end) this.#selector = { start, end };
      else {
        throw new TypeError(
          "Selector must specify either 'prefix' or both 'start' and 'end' key",
        );
      }
    }

    ObjectFreeze(this.#selector);
    this.#pullBatch = pullBatch;
    this.#limit = limit;
    this.#reverse = reverse;
    this.#consistency = consistency;
    this.#batchSize = batchSize;
    this.#cursorGen = cursor ? () => cursor : null;
  }

  get cursor(): string {
    if (this.#cursorGen === null) {
      throw new Error("Cannot get cursor before first iteration");
    }
    return this.#cursorGen();
  }

  async next(): Promise<IteratorResult<Deno.KvEntry<unknown>>> {
    if (
      this.#done ||
      (this.#limit !== undefined && this.#count >= this.#limit)
    ) {
      return { done: true, value: undefined };
    }

    if (!this.#entries?.length && !this.#lastBatch) {
      const batch = await this.#pullBatch(
        this.#selector,
        this.#cursorGen ? this.#cursorGen() : undefined,
        this.#reverse,
        this.#consistency,
      );
      ArrayPrototypeReverse(batch);
      this.#entries = batch;
      if (batch.length < this.#batchSize) {
        this.#lastBatch = true;
      }
    }

    const entry = this.#entries ? ArrayPrototypePop(this.#entries) : undefined;
    if (!entry) {
      this.#done = true;
      this.#cursorGen = () => "";
      return { done: true, value: undefined };
    }

    this.#cursorGen = () => {
      const sel = this.#selector;
      const prefix = ReflectHas(sel, "prefix")
        ? (sel as { prefix: Deno.KvKey }).prefix
        : null;
      const start = ReflectHas(sel, "start")
        ? (sel as { start: Deno.KvKey }).start
        : null;
      const end = ReflectHas(sel, "end")
        ? (sel as { end: Deno.KvKey }).end
        : null;
      const rawSel = selectorFromKv(prefix ?? null, start ?? null, end ?? null);
      const boundaryKey = encodeKvKey(entry.key);
      return encodeCursorFromSelector(rawSel, boundaryKey);
    };
    this.#count++;
    return { done: false, value: entry };
  }

  [SymbolAsyncIterator](): AsyncIterableIterator<Deno.KvEntry<unknown>> {
    return this;
  }
}

// ---------------------------------------------------------------------------
// openKv factory
// ---------------------------------------------------------------------------

async function openKv(path?: string): Promise<Kv> {
  // Distinguish between no argument (undefined) and explicit empty string.
  // undefined - check env vars, then fall back to :memory:
  // "" - reject with "Filename cannot be empty"
  const pathWasProvided = path !== undefined;
  let resolvedPath = path ?? "";

  // Match Rust MultiBackendDbHandler behavior: check env vars without
  // requiring --allow-env (these are internal runtime configuration).
  if (!pathWasProvided && resolvedPath === "") {
    const defaultPath = op_get_env_no_permission_check("DENO_KV_DEFAULT_PATH");
    if (defaultPath) {
      resolvedPath = defaultPath;
    }
  }

  if (resolvedPath !== "") {
    const prefix = op_get_env_no_permission_check("DENO_KV_PATH_PREFIX");
    if (prefix) {
      resolvedPath = prefix + resolvedPath;
    }
  }

  let backend: KvBackend;

  if (
    StringPrototypeStartsWith(resolvedPath, "https://") ||
    StringPrototypeStartsWith(resolvedPath, "http://")
  ) {
    // Remote backend
    const accessToken = op_get_env_no_permission_check(
      "DENO_KV_ACCESS_TOKEN",
    );
    if (!accessToken) {
      throw new Error(
        "Missing DENO_KV_ACCESS_TOKEN environment variable. " +
          "Please set it to your access token from https://dash.deno.com/account.",
      );
    }
    backend = new RemoteKvBackend(resolvedPath, accessToken);
  } else {
    // Local SQLite backend
    if (resolvedPath === "" && !pathWasProvided) {
      // No path and no DENO_KV_DEFAULT_PATH: use in-memory database
      // (matches Rust behavior when path is None and no default_storage_dir)
      resolvedPath = ":memory:";
    } else if (resolvedPath === "") {
      throw new TypeError("Filename cannot be empty");
    }
    if (
      resolvedPath !== ":memory:" &&
      StringPrototypeStartsWith(resolvedPath, ":") &&
      !StringPrototypeStartsWith(resolvedPath, "./") &&
      !StringPrototypeStartsWith(resolvedPath, "../")
    ) {
      throw new TypeError(
        "Filename cannot start with ':' unless prefixed with './'",
      );
    }
    backend = new SqliteKvBackend(resolvedPath);
  }

  return await PromiseResolve(new Kv(backend, kvSymbol));
}

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------

export { AtomicOperation, Kv, KvListIterator, KvU64, openKv };
