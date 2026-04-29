// Copyright 2018-2026 the Deno authors. MIT license.

// SQLite-based local backend for Deno KV, using the node:sqlite API.
// This is a pure-JS rewrite of the Rust denokv_sqlite backend, maintaining
// exact schema and behavioral compatibility.

import { core, primordials } from "ext:core/mod.js";
const {
  ArrayFrom,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  BigInt,
  BigIntAsUintN,
  DataView,
  DataViewPrototypeGetBigUint64,
  DataViewPrototypeSetBigInt64,
  DataViewPrototypeSetBigUint64,
  DateNow,
  JSONParse,
  JSONStringify,
  MathFloor,
  MathRandom,
  Number,
  ObjectPrototypeIsPrototypeOf,
  StringPrototypeCharCodeAt,
  TypeError,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeSet,
  Uint8Array,
  Uint8ArrayPrototype,
} = primordials;

import { crypto } from "ext:deno_crypto/00_crypto.js";
import { DatabaseSync } from "node:sqlite";

// Capture built-ins at load time to prevent monkeypatching
// deno-lint-ignore prefer-primordials
const cryptoRandomUUID = crypto.randomUUID.bind(crypto);

// Value encoding constants (must match denokv_proto)
const VE_V8 = 1;
const VE_LE64 = 2;
const VE_BYTES = 3;

// Default backoff schedule for queue messages (in milliseconds)
const DEFAULT_BACKOFF_SCHEDULE = [100, 1000, 5000, 30000, 60000];

// Deadline timeout for running queue messages (5 seconds)
const MESSAGE_DEADLINE_TIMEOUT_MS = 5000;

// -- Types --

export type KvValue =
  | { kind: "v8"; data: Uint8Array }
  | { kind: "bytes"; data: Uint8Array }
  | { kind: "u64"; value: bigint };

export interface KvEntry {
  key: Uint8Array;
  value: KvValue;
  versionstamp: Uint8Array; // 10 bytes
}

export interface ReadRange {
  start: Uint8Array;
  end: Uint8Array;
  limit: number;
  reverse: boolean;
}

export interface Check {
  key: Uint8Array;
  versionstamp: Uint8Array | null; // null means "key must not exist"
}

export interface Mutation {
  key: Uint8Array;
  kind: MutationKind;
  expireAt: number | null; // milliseconds since epoch, null = no expiry
}

export type MutationKind =
  | { type: "set"; value: KvValue }
  | { type: "delete" }
  | {
    type: "sum";
    value: KvValue;
    minV8: Uint8Array;
    maxV8: Uint8Array;
    clamp: boolean;
  }
  | { type: "min"; value: KvValue }
  | { type: "max"; value: KvValue }
  | { type: "setSuffixVersionstampedKey"; value: KvValue };

export interface Enqueue {
  payload: Uint8Array;
  deadlineMs: number;
  keysIfUndelivered: Uint8Array[];
  backoffSchedule: number[] | null;
}

export interface CommitResult {
  versionstamp: Uint8Array; // 10 bytes
}

// -- SQL Statements --

const STATEMENT_CREATE_MIGRATION_TABLE = `
CREATE TABLE IF NOT EXISTS migration_state(
  k INTEGER NOT NULL PRIMARY KEY,
  version INTEGER NOT NULL
)`;

const MIGRATIONS = [
  // Migration 1: core KV tables
  `CREATE TABLE data_version (
  k INTEGER PRIMARY KEY,
  version INTEGER NOT NULL
);
INSERT INTO data_version (k, version) VALUES (0, 0);
CREATE TABLE kv (
  k BLOB PRIMARY KEY,
  v BLOB NOT NULL,
  v_encoding INTEGER NOT NULL,
  version INTEGER NOT NULL
) WITHOUT ROWID;`,

  // Migration 2: queue tables
  `CREATE TABLE queue (
  ts INTEGER NOT NULL,
  id TEXT NOT NULL,
  data BLOB NOT NULL,
  backoff_schedule TEXT NOT NULL,
  keys_if_undelivered BLOB NOT NULL,
  PRIMARY KEY (ts, id)
);
CREATE TABLE queue_running(
  deadline INTEGER NOT NULL,
  id TEXT NOT NULL,
  data BLOB NOT NULL,
  backoff_schedule TEXT NOT NULL,
  keys_if_undelivered BLOB NOT NULL,
  PRIMARY KEY (deadline, id)
);`,

  // Migration 3: expiration support
  `ALTER TABLE kv ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
ALTER TABLE data_version ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kv ADD COLUMN expiration_ms INTEGER NOT NULL DEFAULT -1;
CREATE INDEX kv_expiration_ms_idx ON kv (expiration_ms);`,
];

const SQL_INC_AND_GET_DATA_VERSION =
  "UPDATE data_version SET version = version + ? WHERE k = 0 RETURNING version";
const SQL_KV_RANGE_SCAN =
  "SELECT k, v, v_encoding, version FROM kv WHERE k >= ? AND k < ? AND (expiration_ms < 0 OR expiration_ms > ?) ORDER BY k ASC LIMIT ?";
const SQL_KV_RANGE_SCAN_REVERSE =
  "SELECT k, v, v_encoding, version FROM kv WHERE k >= ? AND k < ? AND (expiration_ms < 0 OR expiration_ms > ?) ORDER BY k DESC LIMIT ?";
const SQL_KV_POINT_GET_VALUE_ONLY = "SELECT v, v_encoding FROM kv WHERE k = ?";
const SQL_KV_POINT_GET_VERSION_ONLY = "SELECT version FROM kv WHERE k = ?";
const SQL_KV_POINT_SET =
  "INSERT INTO kv (k, v, v_encoding, version, expiration_ms) VALUES (:k, :v, :v_encoding, :version, :expiration_ms) ON CONFLICT(k) DO UPDATE SET v = :v, v_encoding = :v_encoding, version = :version, expiration_ms = :expiration_ms";
const SQL_KV_POINT_DELETE = "DELETE FROM kv WHERE k = ?";
const SQL_DELETE_ALL_EXPIRED =
  "DELETE FROM kv WHERE expiration_ms >= 0 AND expiration_ms <= ?";
const SQL_QUEUE_ADD_READY =
  "INSERT INTO queue (ts, id, data, backoff_schedule, keys_if_undelivered) VALUES(?, ?, ?, ?, ?)";
const SQL_QUEUE_GET_NEXT_READY =
  "SELECT ts, id, data, backoff_schedule, keys_if_undelivered FROM queue WHERE ts <= ? ORDER BY ts LIMIT 1";
const SQL_QUEUE_REMOVE_READY = "DELETE FROM queue WHERE id = ?";
const SQL_QUEUE_ADD_RUNNING =
  "INSERT INTO queue_running (deadline, id, data, backoff_schedule, keys_if_undelivered) VALUES(?, ?, ?, ?, ?)";
const SQL_QUEUE_REMOVE_RUNNING = "DELETE FROM queue_running WHERE id = ?";
const SQL_QUEUE_GET_RUNNING_BY_ID =
  "SELECT deadline, id, data, backoff_schedule, keys_if_undelivered FROM queue_running WHERE id = ?";
const SQL_QUEUE_GET_RUNNING_PAST_DEADLINE =
  "SELECT deadline, id, data, backoff_schedule, keys_if_undelivered FROM queue_running WHERE deadline <= ? ORDER BY deadline";

// -- Helpers --

/**
 * Convert an i64 version number into a 10-byte versionstamp.
 * First 8 bytes = version as big-endian u64, last 2 bytes = 0.
 */
function versionToVersionstamp(version: number | bigint): Uint8Array {
  const buf = new Uint8Array(10);
  const view = new DataView(TypedArrayPrototypeGetBuffer(buf));
  DataViewPrototypeSetBigInt64(view, 0, BigInt(version), false); // big-endian
  // bytes [8] and [9] remain 0
  return buf;
}

/**
 * Compare two versionstamps for equality. Returns true if both are null,
 * or both are non-null with identical bytes.
 */
function versionstampEquals(
  a: Uint8Array | null,
  b: Uint8Array | null,
): boolean {
  if (a === null && b === null) return true;
  if (a === null || b === null) return false;
  if (TypedArrayPrototypeGetLength(a) !== TypedArrayPrototypeGetLength(b)) {
    return false;
  }
  for (let i = 0; i < TypedArrayPrototypeGetLength(a); i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

/**
 * Encode a KvValue into (data, encoding) for storage.
 */
function encodeValue(value: KvValue): { data: Uint8Array; encoding: number } {
  switch (value.kind) {
    case "v8":
      return { data: value.data, encoding: VE_V8 };
    case "bytes":
      return { data: value.data, encoding: VE_BYTES };
    case "u64": {
      const buf = new Uint8Array(8);
      const view = new DataView(TypedArrayPrototypeGetBuffer(buf));
      DataViewPrototypeSetBigUint64(view, 0, value.value, true); // little-endian
      return { data: buf, encoding: VE_LE64 };
    }
  }
}

/**
 * Decode a stored (data, encoding) pair back into a KvValue.
 */
function decodeValue(data: Uint8Array, encoding: number): KvValue {
  switch (encoding) {
    case VE_V8:
      return { kind: "v8", data };
    case VE_BYTES:
      return { kind: "bytes", data };
    case VE_LE64: {
      const view = new DataView(
        TypedArrayPrototypeGetBuffer(data),
        TypedArrayPrototypeGetByteOffset(data),
        TypedArrayPrototypeGetByteLength(data),
      );
      return {
        kind: "u64",
        value: DataViewPrototypeGetBigUint64(view, 0, true),
      };
    }
    default:
      throw new TypeError(`Unknown value encoding: ${encoding}`);
  }
}

/**
 * Write a u64 as a LE64-encoded Uint8Array.
 */
function writeLE64(value: bigint): Uint8Array {
  const buf = new Uint8Array(8);
  const view = new DataView(TypedArrayPrototypeGetBuffer(buf));
  DataViewPrototypeSetBigUint64(view, 0, value, true);
  return buf;
}

/**
 * Generate a random integer in [min, max) range.
 */
function randomInt(min: number, max: number): number {
  return min + MathFloor(MathRandom() * (max - min));
}

/**
 * Generate a UUID v4 string.
 */
function generateUUID(): string {
  return cryptoRandomUUID();
}

/**
 * Hex-encode a Uint8Array to a lowercase hex string.
 */
const HEX_CHARS = "0123456789abcdef";
function hexEncode(bytes: Uint8Array): string {
  let result = "";
  for (let i = 0; i < TypedArrayPrototypeGetLength(bytes); i++) {
    result += HEX_CHARS[bytes[i] >> 4] + HEX_CHARS[bytes[i] & 0xf];
  }
  return result;
}

/**
 * Concatenate two Uint8Arrays.
 */
function concatBytes(a: Uint8Array, b: Uint8Array): Uint8Array {
  const result = new Uint8Array(
    TypedArrayPrototypeGetLength(a) + TypedArrayPrototypeGetLength(b),
  );
  TypedArrayPrototypeSet(result, a, 0);
  TypedArrayPrototypeSet(result, b, TypedArrayPrototypeGetLength(a));
  return result;
}

// -- Main Class --

export class SqliteBackend {
  #db: DatabaseSync;
  #closed = false;

  // Prepared statements (lazily created)
  #stmtRangeScan: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtRangeScanReverse: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtPointGetValueOnly: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtPointGetVersionOnly: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtPointSet: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtPointDelete: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtIncAndGetDataVersion: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtDeleteAllExpired: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtQueueAddReady: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtQueueGetNextReady: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtQueueRemoveReady: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtQueueAddRunning: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtQueueRemoveRunning: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtQueueGetRunningById: ReturnType<DatabaseSync["prepare"]> | null = null;
  #stmtQueueGetRunningPastDeadline: ReturnType<DatabaseSync["prepare"]> | null =
    null;

  constructor(path: string | ":memory:") {
    this.#db = new DatabaseSync(path);

    // Enable WAL mode for better concurrent read/write performance
    this.#db.exec("PRAGMA journal_mode=wal");

    // Run migrations
    this.#runMigrations();

    // Prepare statements
    this.#prepareStatements();
  }

  #runMigrations(): void {
    this.#db.exec(STATEMENT_CREATE_MIGRATION_TABLE);

    // Get current migration version
    const row = this.#db
      .prepare("SELECT version FROM migration_state WHERE k = 0")
      .get() as { version: number } | undefined;
    const currentVersion = row?.version ?? 0;

    for (let i = 0; i < MIGRATIONS.length; i++) {
      const version = i + 1;
      if (version > currentVersion) {
        this.#db.exec(MIGRATIONS[i]);
        this.#db
          .prepare(
            "REPLACE INTO migration_state (k, version) VALUES(?, ?)",
          )
          .run(0, version);
      }
    }
  }

  #prepareStatements(): void {
    this.#stmtRangeScan = this.#db.prepare(SQL_KV_RANGE_SCAN);
    this.#stmtRangeScanReverse = this.#db.prepare(SQL_KV_RANGE_SCAN_REVERSE);
    this.#stmtPointGetValueOnly = this.#db.prepare(SQL_KV_POINT_GET_VALUE_ONLY);
    this.#stmtPointGetVersionOnly = this.#db.prepare(
      SQL_KV_POINT_GET_VERSION_ONLY,
    );
    this.#stmtPointSet = this.#db.prepare(SQL_KV_POINT_SET);
    this.#stmtPointDelete = this.#db.prepare(SQL_KV_POINT_DELETE);
    this.#stmtIncAndGetDataVersion = this.#db.prepare(
      SQL_INC_AND_GET_DATA_VERSION,
    );
    this.#stmtDeleteAllExpired = this.#db.prepare(SQL_DELETE_ALL_EXPIRED);
    this.#stmtQueueAddReady = this.#db.prepare(SQL_QUEUE_ADD_READY);
    this.#stmtQueueGetNextReady = this.#db.prepare(SQL_QUEUE_GET_NEXT_READY);
    this.#stmtQueueRemoveReady = this.#db.prepare(SQL_QUEUE_REMOVE_READY);
    this.#stmtQueueAddRunning = this.#db.prepare(SQL_QUEUE_ADD_RUNNING);
    this.#stmtQueueRemoveRunning = this.#db.prepare(SQL_QUEUE_REMOVE_RUNNING);
    this.#stmtQueueGetRunningById = this.#db.prepare(
      SQL_QUEUE_GET_RUNNING_BY_ID,
    );
    this.#stmtQueueGetRunningPastDeadline = this.#db.prepare(
      SQL_QUEUE_GET_RUNNING_PAST_DEADLINE,
    );
  }

  #ensureOpen(): void {
    if (this.#closed) {
      throw new core.BadResource("Database is closed.");
    }
  }

  /**
   * Perform a snapshot read of one or more key ranges.
   * Each range produces an array of KvEntry results.
   */
  snapshotRead(ranges: ReadRange[]): KvEntry[][] {
    this.#ensureOpen();

    // Wrap in a transaction for a consistent snapshot
    const results: KvEntry[][] = [];
    const now = DateNow();

    this.#db.exec("BEGIN");
    try {
      for (let ri = 0; ri < ranges.length; ri++) {
        const range = ranges[ri];
        const stmt = range.reverse
          ? this.#stmtRangeScanReverse!
          : this.#stmtRangeScan!;
        const rows = stmt.all(
          range.start,
          range.end,
          now,
          range.limit,
        ) as Array<{
          k: Uint8Array;
          v: Uint8Array;
          v_encoding: number;
          version: number | bigint;
        }>;

        const entries: KvEntry[] = [];
        for (let j = 0; j < rows.length; j++) {
          const row = rows[j];
          ArrayPrototypePush(entries, {
            key: asUint8Array(row.k),
            value: decodeValue(asUint8Array(row.v), row.v_encoding),
            versionstamp: versionToVersionstamp(row.version),
          });
        }
        ArrayPrototypePush(results, entries);
      }

      this.#db.exec("COMMIT");
    } catch (e) {
      this.#db.exec("ROLLBACK");
      throw e;
    }

    return results;
  }

  /**
   * Perform an atomic check-modify-write operation.
   * Returns a CommitResult on success, or null if a check failed.
   */
  atomicWrite(
    checks: Check[],
    mutations: Mutation[],
    enqueues: Enqueue[],
  ): CommitResult | null {
    this.#ensureOpen();

    this.#db.exec("BEGIN IMMEDIATE");
    try {
      const now = DateNow();

      // 1. Delete expired entries
      this.#stmtDeleteAllExpired!.run(now);

      // 2. Perform checks
      for (let ci = 0; ci < checks.length; ci++) {
        const check = checks[ci];
        const row = this.#stmtPointGetVersionOnly!.get(check.key) as
          | { version: number | bigint }
          | undefined;
        const realVersionstamp: Uint8Array | null = row
          ? versionToVersionstamp(row.version)
          : null;

        if (!versionstampEquals(realVersionstamp, check.versionstamp)) {
          this.#db.exec("ROLLBACK");
          return null;
        }
      }

      // 3. Increment data_version by a random amount [1, 10)
      const increment = randomInt(1, 10);
      const versionRow = this.#stmtIncAndGetDataVersion!.get(increment) as {
        version: number | bigint;
      };
      const version = versionRow.version;
      const newVersionstamp = versionToVersionstamp(version);

      // 4. Apply mutations
      for (let mi = 0; mi < mutations.length; mi++) {
        this.#applyMutation(mutations[mi], version, newVersionstamp);
      }

      // 5. Enqueue messages
      for (let ei = 0; ei < enqueues.length; ei++) {
        const enqueue = enqueues[ei];
        const id = generateUUID();
        const backoffSchedule = JSONStringify(
          enqueue.backoffSchedule ?? DEFAULT_BACKOFF_SCHEDULE,
        );
        const keysIfUndelivered = JSONStringify(
          ArrayPrototypeMap(
            enqueue.keysIfUndelivered,
            (k: Uint8Array) => ArrayFrom(k),
          ),
        );

        this.#stmtQueueAddReady!.run(
          enqueue.deadlineMs,
          id,
          enqueue.payload,
          backoffSchedule,
          keysIfUndelivered,
        );
      }

      this.#db.exec("COMMIT");
      return { versionstamp: newVersionstamp };
    } catch (e) {
      try {
        this.#db.exec("ROLLBACK");
      } catch {
        // Ignore rollback errors
      }
      throw e;
    }
  }

  #applyMutation(
    mutation: Mutation,
    version: number | bigint,
    newVersionstamp: Uint8Array,
  ): void {
    const expirationMs = mutation.expireAt ?? -1;

    switch (mutation.kind.type) {
      case "set": {
        const { data, encoding } = encodeValue(mutation.kind.value);
        this.#stmtPointSet!.run({
          ":k": mutation.key,
          ":v": data,
          ":v_encoding": encoding,
          ":version": version,
          ":expiration_ms": expirationMs,
        });
        break;
      }

      case "delete": {
        this.#stmtPointDelete!.run(mutation.key);
        break;
      }

      case "sum": {
        this.#applySumMutation(
          mutation.key,
          mutation.kind.value,
          mutation.kind.minV8,
          mutation.kind.maxV8,
          mutation.kind.clamp,
          version,
        );
        break;
      }

      case "min": {
        this.#applyLE64Mutation(
          mutation.key,
          "min",
          mutation.kind.value,
          version,
          (a, b) => (a < b ? a : b),
        );
        break;
      }

      case "max": {
        this.#applyLE64Mutation(
          mutation.key,
          "max",
          mutation.kind.value,
          version,
          (a, b) => (a > b ? a : b),
        );
        break;
      }

      case "setSuffixVersionstampedKey": {
        // Build the suffix: 0x02 + hex(versionstamp) + 0x00
        const hexStr = hexEncode(newVersionstamp);
        const suffix = new Uint8Array(22);
        suffix[0] = 0x02;
        for (let i = 0; i < 20; i++) {
          suffix[1 + i] = StringPrototypeCharCodeAt(hexStr, i);
        }
        // suffix[21] is already 0x00

        const key = concatBytes(mutation.key, suffix);
        const { data, encoding } = encodeValue(mutation.kind.value);
        this.#stmtPointSet!.run({
          ":k": key,
          ":v": data,
          ":v_encoding": encoding,
          ":version": version,
          ":expiration_ms": expirationMs,
        });
        break;
      }
    }
  }

  /**
   * Apply a LE64 mutation (min/max) on a u64 value.
   * If the key does not exist, the operand becomes the new value.
   */
  #applyLE64Mutation(
    key: Uint8Array,
    opName: string,
    operand: KvValue,
    version: number | bigint,
    mutate: (existing: bigint, operand: bigint) => bigint,
  ): void {
    if (operand.kind !== "u64") {
      throw new TypeError(
        `Failed to perform '${opName}' mutation on a non-U64 operand`,
      );
    }

    const row = this.#stmtPointGetValueOnly!.get(key) as
      | { v: Uint8Array; v_encoding: number }
      | undefined;

    let newValue: bigint;
    if (row) {
      const existing = decodeValue(asUint8Array(row.v), row.v_encoding);
      if (existing.kind !== "u64") {
        throw new TypeError(
          `Failed to perform '${opName}' mutation on a non-U64 value in the database`,
        );
      }
      newValue = mutate(existing.value, operand.value);
    } else {
      newValue = operand.value;
    }

    const data = writeLE64(newValue);
    this.#stmtPointSet!.run({
      ":k": key,
      ":v": data,
      ":v_encoding": VE_LE64,
      ":version": version,
      ":expiration_ms": -1,
    });
  }

  /**
   * Apply a sum mutation. For U64 operands, uses wrapping addition.
   * For V8 operands, this is a more complex operation that is not yet
   * supported in this JS backend (V8 sum requires V8 value deserialization).
   */
  #applySumMutation(
    key: Uint8Array,
    operand: KvValue,
    minV8: Uint8Array,
    maxV8: Uint8Array,
    clamp: boolean,
    version: number | bigint,
  ): void {
    // For U64 operands, use wrapping addition (matching the Rust backend)
    if (operand.kind === "u64") {
      // Check if existing value is a different type
      const row = this.#stmtPointGetValueOnly!.get(key) as
        | { v: Uint8Array; v_encoding: number }
        | undefined;
      if (row) {
        const existing = decodeValue(asUint8Array(row.v), row.v_encoding);
        if (existing.kind !== "u64") {
          throw new TypeError(
            `Failed to perform 'sum' mutation on a non-U64 value in the database`,
          );
        }
      }
      this.#applyLE64Mutation(
        key,
        "sum",
        operand,
        version,
        (a, b) => BigIntAsUintN(64, a + b),
      );
      return;
    }

    // V8 sum (Number/BigInt operand)
    const row = this.#stmtPointGetValueOnly!.get(key) as
      | { v: Uint8Array; v_encoding: number }
      | undefined;

    if (!row) {
      // Key doesn't exist - just set the operand value
      const { data, encoding } = encodeValue(operand);
      this.#stmtPointSet!.run({
        ":k": key,
        ":v": data,
        ":v_encoding": encoding,
        ":version": version,
        ":expiration_ms": -1,
      });
      return;
    }

    const existing = decodeValue(asUint8Array(row.v), row.v_encoding);
    if (existing.kind === "u64") {
      throw new TypeError(
        "Cannot sum KvU64 with Number",
      );
    }

    // Both are V8: deserialize, add, clamp, re-serialize
    const existingVal = core.deserialize(existing.data, { forStorage: true });
    const operandVal = core.deserialize(operand.data, { forStorage: true });
    let result = existingVal + operandVal;

    if (clamp) {
      const minVal = TypedArrayPrototypeGetLength(minV8) > 0
        ? core.deserialize(minV8, { forStorage: true })
        : undefined;
      const maxVal = TypedArrayPrototypeGetLength(maxV8) > 0
        ? core.deserialize(maxV8, { forStorage: true })
        : undefined;
      if (minVal !== undefined && result < minVal) result = minVal;
      if (maxVal !== undefined && result > maxVal) result = maxVal;
    }

    const serialized = core.serialize(result, { forStorage: true });
    this.#stmtPointSet!.run({
      ":k": key,
      ":v": serialized,
      ":v_encoding": VE_V8,
      ":version": version,
      ":expiration_ms": -1,
    });
  }

  /**
   * Dequeue the next ready message from the queue.
   * Returns the payload and ID, or null if no messages are ready.
   */
  dequeueNextMessage(): { payload: Uint8Array; id: string } | null {
    this.#ensureOpen();

    const now = DateNow();

    // Clean up messages stuck in the running queue past their deadline
    this.#cleanupRunningQueue(now);

    this.#db.exec("BEGIN IMMEDIATE");
    try {
      const row = this.#stmtQueueGetNextReady!.get(now) as
        | {
          ts: number;
          id: string;
          data: Uint8Array;
          backoff_schedule: string;
          keys_if_undelivered: string;
        }
        | undefined;

      if (!row) {
        this.#db.exec("COMMIT");
        return null;
      }

      // Remove from ready queue
      this.#stmtQueueRemoveReady!.run(row.id);

      // Add to running queue with deadline
      const deadline = Number(row.ts) + MESSAGE_DEADLINE_TIMEOUT_MS;
      this.#stmtQueueAddRunning!.run(
        deadline,
        row.id,
        row.data,
        row.backoff_schedule,
        row.keys_if_undelivered,
      );

      this.#db.exec("COMMIT");
      return { payload: asUint8Array(row.data), id: row.id };
    } catch (e) {
      try {
        this.#db.exec("ROLLBACK");
      } catch {
        // Ignore rollback errors
      }
      throw e;
    }
  }

  /**
   * Finish processing a dequeued message.
   * If success is true, removes the message. If false, applies backoff and requeues.
   */
  finishMessage(id: string, success: boolean): void {
    this.#ensureOpen();

    this.#db.exec("BEGIN IMMEDIATE");
    try {
      if (success) {
        this.#stmtQueueRemoveRunning!.run(id);
        this.#db.exec("COMMIT");
        return;
      }

      // Failure: requeue with backoff
      this.#requeueMessage(id);
      this.#db.exec("COMMIT");
    } catch (e) {
      try {
        this.#db.exec("ROLLBACK");
      } catch {
        // Ignore rollback errors
      }
      throw e;
    }
  }

  #cleanupRunningQueue(now: number): void {
    this.#db.exec("BEGIN IMMEDIATE");
    try {
      const rows = this.#stmtQueueGetRunningPastDeadline!.all(now) as Array<{
        deadline: number;
        id: string;
        data: Uint8Array;
        backoff_schedule: string;
        keys_if_undelivered: string;
      }>;
      for (let i = 0; i < rows.length; i++) {
        this.#requeueMessage(rows[i].id);
      }
      this.#db.exec("COMMIT");
    } catch (e) {
      try {
        this.#db.exec("ROLLBACK");
      } catch {
        // Ignore rollback errors
      }
      throw e;
    }
  }

  #requeueMessage(id: string): void {
    const row = this.#stmtQueueGetRunningById!.get(id) as
      | {
        deadline: number;
        id: string;
        data: Uint8Array;
        backoff_schedule: string;
        keys_if_undelivered: string;
      }
      | undefined;

    if (!row) {
      return;
    }

    const backoffSchedule: number[] | null = JSONParse(row.backoff_schedule);
    const schedule = backoffSchedule ?? [];

    const now = DateNow();

    if (schedule.length > 0) {
      // Requeue with the first backoff delay, rest is remaining schedule
      const newTs = now + schedule[0];
      const newBackoffSchedule = JSONStringify(
        ArrayPrototypeSlice(schedule, 1),
      );

      this.#stmtQueueAddReady!.run(
        newTs,
        row.id,
        row.data,
        newBackoffSchedule,
        row.keys_if_undelivered,
      );
    } else {
      // No more retries. Write to keys_if_undelivered.
      const keysIfUndelivered: number[][] = JSONParse(
        row.keys_if_undelivered,
      );
      if (keysIfUndelivered.length > 0) {
        const increment = randomInt(1, 10);
        const versionRow = this.#stmtIncAndGetDataVersion!.get(increment) as {
          version: number | bigint;
        };
        const version = versionRow.version;

        for (let ki = 0; ki < keysIfUndelivered.length; ki++) {
          const key = new Uint8Array(keysIfUndelivered[ki]);
          this.#stmtPointSet!.run({
            ":k": key,
            ":v": row.data,
            ":v_encoding": VE_V8,
            ":version": version,
            ":expiration_ms": -1,
          });
        }
      }
    }

    // Remove from running
    this.#stmtQueueRemoveRunning!.run(id);
  }

  /**
   * Close the database connection.
   */
  close(): void {
    if (!this.#closed) {
      this.#closed = true;
      this.#db.close();
    }
  }
}

/**
 * Ensure a value is a Uint8Array. node:sqlite may return Buffer objects;
 * this normalizes them.
 */
function asUint8Array(value: Uint8Array | ArrayBuffer): Uint8Array {
  if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, value)) {
    return value as Uint8Array;
  }
  return new Uint8Array(value);
}
