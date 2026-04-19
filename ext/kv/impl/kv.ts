// Copyright 2018-2026 the Deno authors. MIT license.

// Pure JS implementation of the Deno KV API.
// Replaces the Rust ops with JS backends (SQLite for local, HTTP for remote).
//
// NOTE: This module still depends on `core.serialize` / `core.deserialize`
// from deno_core for V8 structured clone of arbitrary JS values.
// That is the only native dependency remaining.

import { core } from "ext:core/mod.js";
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

// ---------------------------------------------------------------------------
// Base64url for cursor encoding (matching the Rust backend)
// ---------------------------------------------------------------------------

const BASE64URL =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

function base64urlEncode(data: Uint8Array): string {
  let result = "";
  let i = 0;
  while (i < data.length) {
    const b0 = data[i++];
    const b1 = i < data.length ? data[i++] : 0;
    const b2 = i < data.length ? data[i++] : 0;
    const n = (b0 << 16) | (b1 << 8) | b2;
    result += BASE64URL[(n >> 18) & 63];
    result += BASE64URL[(n >> 12) & 63];
    if (i - 1 <= data.length) result += BASE64URL[(n >> 6) & 63];
    if (i <= data.length) result += BASE64URL[n & 63];
  }
  // Padding with '='
  const pad = data.length % 3;
  if (pad === 1) result += "==";
  else if (pad === 2) result += "=";
  return result;
}

function base64urlDecode(str: string): Uint8Array {
  const s = str.replace(/=+$/, "");
  const lookup = new Uint8Array(128);
  for (let i = 0; i < BASE64URL.length; i++) {
    lookup[BASE64URL.charCodeAt(i)] = i;
  }
  const bytes: number[] = [];
  for (let i = 0; i < s.length; i += 4) {
    const a = lookup[s.charCodeAt(i)];
    const b = lookup[s.charCodeAt(i + 1)] ?? 0;
    const c = lookup[s.charCodeAt(i + 2)] ?? 0;
    const d = lookup[s.charCodeAt(i + 3)] ?? 0;
    const n = (a << 18) | (b << 12) | (c << 6) | d;
    bytes.push((n >> 16) & 0xff);
    if (i + 2 < s.length) bytes.push((n >> 8) & 0xff);
    if (i + 3 < s.length) bytes.push(n & 0xff);
  }
  return new Uint8Array(bytes);
}

// ---------------------------------------------------------------------------
// Value serialization — uses V8 structured clone for arbitrary JS values
// ---------------------------------------------------------------------------

interface RawValue {
  kind: "v8" | "bytes" | "u64";
  value: Uint8Array | bigint;
}

function serializeValue(value: unknown): RawValue {
  if (value instanceof Uint8Array) {
    return { kind: "bytes", value };
  } else if (value instanceof KvU64) {
    return { kind: "u64", value: value.value };
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
  let s = "";
  for (let i = 0; i < buf.length; i++) {
    s += buf[i].toString(16).padStart(2, "0");
  }
  return s;
}

function hexDecode(s: string): Uint8Array {
  const out = new Uint8Array(s.length >> 1);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(s.substring(i * 2, i * 2 + 2), 16);
  }
  return out;
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
        value: new DataView(
          e.value.buffer,
          e.value.byteOffset,
          e.value.byteLength,
        ).getBigUint64(0, true),
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
      new DataView(buf.buffer).setBigUint64(0, value.value, true);
      return { data: buf, encoding: ValueEncoding.VE_LE64 };
    }
  }
}

// ---------------------------------------------------------------------------
// Key encoding helpers
// ---------------------------------------------------------------------------

function encodeKvKey(key: Deno.KvKey): Uint8Array {
  return encodeKey(
    kvKeyToKeyParts(key as (string | number | bigint | Uint8Array | boolean)[]),
  );
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
      !startsWith(startBytes, prefixBytes) ||
      startBytes.length === prefixBytes.length
    ) {
      throw new TypeError("Start key is not in the keyspace defined by prefix");
    }
    return { kind: "prefixed", prefix: prefixBytes, start: startBytes };
  }
  if (prefixBytes && !startBytes && endBytes) {
    if (
      !startsWith(endBytes, prefixBytes) ||
      endBytes.length === prefixBytes.length
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
    const endComputed = new Uint8Array(startBytes.length + 1);
    endComputed.set(startBytes);
    endComputed[startBytes.length] = 0;
    return { kind: "range", start: startBytes, end: endComputed };
  }
  throw new TypeError(
    "Selector must specify either 'prefix' or both 'start' and 'end' key",
  );
}

function selectorRangeStart(sel: RawSelector): Uint8Array {
  if (sel.start) return sel.start;
  if (sel.prefix) {
    const r = new Uint8Array(sel.prefix.length + 1);
    r.set(sel.prefix);
    r[sel.prefix.length] = 0x00;
    return r;
  }
  throw new TypeError("Invalid selector");
}

function selectorRangeEnd(sel: RawSelector): Uint8Array {
  if (sel.end) return sel.end;
  if (sel.prefix) {
    const r = new Uint8Array(sel.prefix.length + 1);
    r.set(sel.prefix);
    r[sel.prefix.length] = 0xff;
    return r;
  }
  throw new TypeError("Invalid selector");
}

function selectorCommonPrefix(sel: RawSelector): Uint8Array {
  if (sel.prefix) return sel.prefix;
  const start = sel.start!;
  const end = sel.end!;
  let i = 0;
  while (i < start.length && i < end.length && start[i] === end[i]) i++;
  return start.slice(0, i);
}

function encodeCursorFromSelector(
  sel: RawSelector,
  boundaryKey: Uint8Array,
): string {
  const common = selectorCommonPrefix(sel);
  if (!startsWith(boundaryKey, common)) {
    throw new TypeError("Invalid boundary key");
  }
  return base64urlEncode(boundaryKey.slice(common.length));
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

  let firstKey: Uint8Array;
  let lastKey: Uint8Array;

  if (reverse) {
    firstKey = selectorRangeStart(sel);
    lastKey = new Uint8Array(common.length + cursorBytes.length);
    lastKey.set(common);
    lastKey.set(cursorBytes, common.length);
  } else {
    firstKey = new Uint8Array(common.length + cursorBytes.length + 1);
    firstKey.set(common);
    firstKey.set(cursorBytes, common.length);
    firstKey[common.length + cursorBytes.length] = 0;
    lastKey = selectorRangeEnd(sel);
  }

  return { start: firstKey, end: lastKey };
}

// ---------------------------------------------------------------------------
// Byte comparison helpers
// ---------------------------------------------------------------------------

function startsWith(a: Uint8Array, prefix: Uint8Array): boolean {
  if (a.length < prefix.length) return false;
  for (let i = 0; i < prefix.length; i++) {
    if (a[i] !== prefix[i]) return false;
  }
  return true;
}

function compareBytes(a: Uint8Array, b: Uint8Array): number {
  const len = Math.min(a.length, b.length);
  for (let i = 0; i < len; i++) {
    if (a[i] < b[i]) return -1;
    if (a[i] > b[i]) return 1;
  }
  return a.length - b.length;
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
    return Promise.resolve(this.#backend.snapshotRead(ranges));
  }

  atomicWrite(
    checks: SqliteCheck[],
    mutations: SqliteMutation[],
    enqueues: SqliteEnqueue[],
  ): Promise<CommitResult | null> {
    return Promise.resolve(
      this.#backend.atomicWrite(checks, mutations, enqueues),
    );
  }

  dequeueNextMessage(): Promise<{ payload: Uint8Array; id: string } | null> {
    return Promise.resolve(this.#backend.dequeueNextMessage());
  }

  finishMessage(id: string, success: boolean): Promise<void> {
    this.#backend.finishMessage(id, success);
    return Promise.resolve();
  }

  watch(keys: Uint8Array[]): ReadableStream {
    // SQLite watch: poll-based implementation
    // For now, return a stream that polls every 500ms
    const backend = this.#backend;
    const lastVersionstamps: (string | null)[] = keys.map(() => null);
    let closed = false;

    return new ReadableStream({
      async pull(controller) {
        while (!closed) {
          const ranges = keys.map((k) => ({
            start: k,
            end: new Uint8Array([...k, 0]),
            limit: 1,
            reverse: false,
          }));

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
              entries.push({
                key: decodedKey,
                value: deserializeRawValue(kvValueToRawValue(entry.value)),
                versionstamp: hexEncode(entry.versionstamp),
              });
            } else {
              entries.push({
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
    const protoRanges: ProtoReadRange[] = ranges.map((r) => ({
      start: r.start,
      end: r.end,
      limit: r.limit,
      reverse: r.reverse,
    }));
    const result = await this.#backend.snapshotRead(protoRanges, consistency);
    return result.map((entries) => entries.map(remoteEntryToKvEntry));
  }

  async atomicWrite(
    checks: SqliteCheck[],
    mutations: SqliteMutation[],
    enqueues: SqliteEnqueue[],
  ): Promise<CommitResult | null> {
    const protoChecks: ProtoCheck[] = checks.map((c) => ({
      key: c.key,
      versionstamp: c.versionstamp ?? new Uint8Array(0),
    }));

    const protoMutations: ProtoMutation[] = mutations.map((m) => {
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
    });

    const protoEnqueues: ProtoEnqueue[] = enqueues.map((e) => ({
      payload: e.payload,
      deadlineMs: BigInt(e.deadlineMs),
      keysIfUndelivered: e.keysIfUndelivered,
      backoffSchedule: e.backoffSchedule ?? [],
    }));

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
    return Promise.resolve(null);
  }

  finishMessage(_id: string, _success: boolean): Promise<void> {
    // Queue operations not supported on remote backend
    return Promise.resolve();
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
    Object.freeze(this);
  }

  valueOf(): bigint {
    return this.value;
  }

  toString(): string {
    return this.value.toString();
  }

  get [Symbol.toStringTag](): string {
    return "Deno.KvU64";
  }
}

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
  if (Number.isNaN(delay)) throw new TypeError("Delay cannot be NaN");
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
      Number.isNaN(schedule[i])
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

  constructor(backend: KvBackend, symbol: symbol) {
    if (kvSymbol !== symbol) {
      throw new TypeError(
        "Deno.Kv can not be constructed: use Deno.openKv instead",
      );
    }
    this.#backend = backend;
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
    const endKey = new Uint8Array(encodedKey.length + 1);
    endKey.set(encodedKey);
    endKey[encodedKey.length] = 0;

    const [entries] = await this.#backend.snapshotRead(
      [{ start: encodedKey, end: endKey, limit: 1, reverse: false }],
      opts?.consistency ?? "strong",
    );

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
    const ranges: ReadRange[] = keys.map((key) => {
      const encodedKey = encodeKvKey(key);
      const endKey = new Uint8Array(encodedKey.length + 1);
      endKey.set(encodedKey);
      endKey[encodedKey.length] = 0;
      return { start: encodedKey, end: endKey, limit: 1, reverse: false };
    });

    const results = await this.#backend.snapshotRead(
      ranges,
      opts?.consistency ?? "strong",
    );

    return results.map((entries, i) => {
      if (!entries.length) {
        return { key: keys[i], value: null, versionstamp: null };
      }
      const e = entries[0];
      return {
        key: decodeKvKeyBytes(e.key),
        value: deserializeRawValue(kvValueToRawValue(e.value)),
        versionstamp: hexEncode(e.versionstamp),
      };
    });
  }

  async set(
    key: Deno.KvKey,
    value: unknown,
    options?: { expireIn?: number },
  ): Promise<Deno.KvCommitResult> {
    const result = await doAtomicWrite(
      this.#backend,
      [],
      [
        {
          key: encodeKvKey(key),
          kind: {
            type: "set",
            value: rawValueToKvValue(serializeValue(value)),
          },
          expireAt: options?.expireIn ? Date.now() + options.expireIn : null,
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
    } = {},
  ): KvListIterator {
    if (options.limit !== undefined && options.limit <= 0) {
      throw new Error(`Limit must be positive: received ${options.limit}`);
    }

    let batchSize = options.batchSize ?? (options.limit ?? 100);
    if (batchSize <= 0) throw new Error("batchSize must be positive");
    if (options.batchSize === undefined && batchSize > 500) batchSize = 500;

    const backend = this.#backend;
    const pullBatch = async (
      sel: Deno.KvListSelector,
      cursor: string | undefined,
      reverse: boolean,
      consistency: Deno.KvConsistencyLevel,
    ): Promise<Deno.KvEntry<unknown>[]> => {
      const prefix = "prefix" in sel ? sel.prefix : null;
      const start = "start" in sel ? sel.start : null;
      const end = "end" in sel ? sel.end : null;

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

      const [entries] = await backend.snapshotRead(
        [{ start: rangeStart, end: rangeEnd, limit: batchSize, reverse }],
        consistency,
      );

      return entries.map((e) => ({
        key: decodeKvKeyBytes(e.key),
        value: deserializeRawValue(kvValueToRawValue(e.value)),
        versionstamp: hexEncode(e.versionstamp),
      }));
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
        deadlineMs: Date.now() + (opts?.delay ?? 0),
        keysIfUndelivered: (opts?.keysIfUndelivered ?? []).map(encodeKvKey),
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

    while (!this.#isClosed) {
      const next = await this.#backend.dequeueNextMessage();
      if (next === null) {
        if (this.#isClosed) break;
        // Poll interval when no messages
        await new Promise((r) => setTimeout(r, 1000));
        continue;
      }

      const deserialized = core.deserialize(next.payload, {
        forStorage: true,
      });

      let success = false;
      try {
        const result = handler(deserialized);
        if (
          result && typeof (result as Promise<void>).then === "function"
        ) {
          await result;
        }
        success = true;
      } catch (error) {
        console.error("Exception in queue handler", error);
      } finally {
        await this.#backend.finishMessage(next.id, success);
      }
    }
  }

  watch(
    keys: Deno.KvKey[],
    options: { raw?: boolean } = {},
  ): ReadableStream<Deno.KvEntryMaybe<unknown>[]> {
    const encodedKeys = keys.map(encodeKvKey);
    const raw = options.raw ?? false;
    const rawStream = this.#backend.watch(encodedKeys);

    // For SQLite backend, the stream already produces decoded entries
    // For remote, we need to transform the protobuf entries
    // The Kv.watch() method in the original code does deduplication
    // based on versionstamp changes — replicate that here.
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
                key: [...keys[i]],
                value: null,
                versionstamp: null,
              };
            } else {
              lastEntries[i] = update;
            }
          }
          if (!changed && !raw) continue;
          controller.enqueue(lastEntries.map((e) => ({ ...e! })));
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
  }

  [Symbol.dispose](): void {
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
    for (const check of checks) {
      this.#checks.push({
        key: encodeKvKey(check.key),
        versionstamp: check.versionstamp ? hexDecode(check.versionstamp) : null,
      });
    }
    return this;
  }

  mutate(...mutations: Deno.KvMutation[]): this {
    for (const m of mutations) {
      const key = encodeKvKey(m.key);
      let kind: MutationKind;

      switch (m.type) {
        case "delete":
          kind = { type: "delete" };
          break;
        case "set":
          kind = {
            type: "set",
            value: rawValueToKvValue(serializeValue(m.value)),
          };
          break;
        case "sum":
          kind = {
            type: "sum",
            value: rawValueToKvValue(serializeValue(m.value)),
            minV8: new Uint8Array(0),
            maxV8: new Uint8Array(0),
            clamp: false,
          };
          break;
        case "min":
          kind = {
            type: "min",
            value: rawValueToKvValue(serializeValue(m.value)),
          };
          break;
        case "max":
          kind = {
            type: "max",
            value: rawValueToKvValue(serializeValue(m.value)),
          };
          break;
        default:
          throw new TypeError("Invalid mutation type");
      }

      this.#mutations.push({
        key,
        kind,
        expireAt: ("expireIn" in m && typeof m.expireIn === "number")
          ? Date.now() + m.expireIn
          : null,
      });
    }
    return this;
  }

  sum(key: Deno.KvKey, n: bigint): this {
    this.#mutations.push({
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
    this.#mutations.push({
      key: encodeKvKey(key),
      kind: { type: "min", value: { kind: "u64", value: new KvU64(n).value } },
      expireAt: null,
    });
    return this;
  }

  max(key: Deno.KvKey, n: bigint): this {
    this.#mutations.push({
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
      actualKey = key.slice(0, -1);
      mutationType = "setSuffixVersionstampedKey";
    }

    this.#mutations.push({
      key: encodeKvKey(actualKey),
      kind: {
        type: mutationType,
        value: rawValueToKvValue(serializeValue(value)),
      },
      expireAt: options?.expireIn ? Date.now() + options.expireIn : null,
    });
    return this;
  }

  delete(key: Deno.KvKey): this {
    this.#mutations.push({
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
    this.#enqueues.push({
      payload: core.serialize(message, { forStorage: true }),
      deadlineMs: Date.now() + (opts?.delay ?? 0),
      keysIfUndelivered: (opts?.keysIfUndelivered ?? []).map(encodeKvKey),
      backoffSchedule: opts?.backoffSchedule ?? null,
    });
    return this;
  }

  async commit(): Promise<Deno.KvCommitResult | Deno.KvCommitError> {
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

class KvListIterator implements AsyncIterableIterator<Deno.KvEntry<unknown>> {
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
    // Validate and freeze selector
    let prefix: Deno.KvKey | undefined;
    let start: Deno.KvKey | undefined;
    let end: Deno.KvKey | undefined;

    if ("prefix" in selector && selector.prefix !== undefined) {
      prefix = Object.freeze([...selector.prefix]);
    }
    if ("start" in selector && selector.start !== undefined) {
      start = Object.freeze([...selector.start]);
    }
    if ("end" in selector && selector.end !== undefined) {
      end = Object.freeze([...selector.end]);
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

    Object.freeze(this.#selector);
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
      batch.reverse();
      this.#entries = batch;
      if (batch.length < this.#batchSize) {
        this.#lastBatch = true;
      }
    }

    const entry = this.#entries?.pop();
    if (!entry) {
      this.#done = true;
      this.#cursorGen = () => "";
      return { done: true, value: undefined };
    }

    this.#cursorGen = () => {
      const sel = this.#selector;
      const prefix = "prefix" in sel ? sel.prefix : null;
      const start = "start" in sel ? sel.start : null;
      const end = "end" in sel ? sel.end : null;
      const rawSel = selectorFromKv(prefix ?? null, start ?? null, end ?? null);
      const boundaryKey = encodeKvKey(entry.key);
      return encodeCursorFromSelector(rawSel, boundaryKey);
    };
    this.#count++;
    return { done: false, value: entry };
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<Deno.KvEntry<unknown>> {
    return this;
  }
}

// ---------------------------------------------------------------------------
// openKv factory
// ---------------------------------------------------------------------------

function openKv(path?: string): Promise<Kv> {
  const resolvedPath = path ?? "";

  let backend: KvBackend;

  if (
    resolvedPath.startsWith("https://") ||
    resolvedPath.startsWith("http://")
  ) {
    // Remote backend
    const accessToken = Deno.env.get("DENO_KV_ACCESS_TOKEN");
    if (!accessToken) {
      throw new Error(
        "Missing DENO_KV_ACCESS_TOKEN environment variable. " +
          "Please set it to your access token from https://dash.deno.com/account.",
      );
    }
    backend = new RemoteKvBackend(resolvedPath, accessToken);
  } else {
    // Local SQLite backend
    let dbPath: string;
    if (resolvedPath === "" || resolvedPath === ":memory:") {
      dbPath = ":memory:";
    } else {
      dbPath = resolvedPath;
    }
    backend = new SqliteKvBackend(dbPath);
  }

  return Promise.resolve(new Kv(backend, kvSymbol));
}

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------

export { AtomicOperation, Kv, KvListIterator, KvU64, openKv };
