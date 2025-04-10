// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  isPromise,
} = core;
import {
  op_kv_atomic_write,
  op_kv_database_open,
  op_kv_dequeue_next_message,
  op_kv_encode_cursor,
  op_kv_finish_dequeued_message,
  op_kv_snapshot_read,
  op_kv_watch,
  op_kv_watch_next,
} from "ext:core/ops";
const {
  ArrayFrom,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeReverse,
  ArrayPrototypeSlice,
  AsyncGeneratorPrototype,
  BigInt,
  BigIntPrototypeToString,
  Error,
  NumberIsNaN,
  Object,
  ObjectFreeze,
  ObjectGetPrototypeOf,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  RangeError,
  SafeMap,
  SafeMapIterator,
  StringPrototypeReplace,
  Symbol,
  SymbolAsyncIterator,
  SymbolFor,
  SymbolToStringTag,
  TypeError,
  TypedArrayPrototypeGetSymbolToStringTag,
} = primordials;

import { SymbolDispose } from "ext:deno_web/00_infra.js";
import { ReadableStream } from "ext:deno_web/06_streams.js";

const encodeCursor: (
  selector: [Deno.KvKey | null, Deno.KvKey | null, Deno.KvKey | null],
  boundaryKey: Deno.KvKey,
) => string = (selector, boundaryKey) =>
  op_kv_encode_cursor(selector, boundaryKey);

async function openKv(path: string) {
  const rid = await op_kv_database_open(path);
  return new Kv(rid, kvSymbol);
}

const maxQueueDelay = 30 * 24 * 60 * 60 * 1000;

function validateQueueDelay(delay: number) {
  if (delay < 0) {
    throw new TypeError(`Delay must be >= 0: received ${delay}`);
  }
  if (delay > maxQueueDelay) {
    throw new TypeError(
      `Delay cannot be greater than 30 days: received ${delay}`,
    );
  }
  if (NumberIsNaN(delay)) {
    throw new TypeError("Delay cannot be NaN");
  }
}

const maxQueueBackoffIntervals = 5;
const maxQueueBackoffInterval = 60 * 60 * 1000;

function validateBackoffSchedule(backoffSchedule: number[]) {
  if (backoffSchedule.length > maxQueueBackoffIntervals) {
    throw new TypeError(
      `Invalid backoffSchedule, max ${maxQueueBackoffIntervals} intervals allowed`,
    );
  }
  for (let i = 0; i < backoffSchedule.length; ++i) {
    const interval = backoffSchedule[i];
    if (
      interval < 0 || interval > maxQueueBackoffInterval ||
      NumberIsNaN(interval)
    ) {
      throw new TypeError(
        `Invalid backoffSchedule, interval at index ${i} is invalid`,
      );
    }
  }
}

interface RawKvEntry {
  key: Deno.KvKey;
  value: RawValue;
  versionstamp: string;
}

type RawValue = {
  kind: "v8";
  value: Uint8Array;
} | {
  kind: "bytes";
  value: Uint8Array;
} | {
  kind: "u64";
  value: bigint;
};

const kvSymbol = Symbol("KvRid");
const commitVersionstampSymbol = Symbol("KvCommitVersionstamp");

class Kv {
  #rid: number;
  #isClosed: boolean;

  constructor(rid: number = undefined, symbol: symbol = undefined) {
    if (kvSymbol !== symbol) {
      throw new TypeError(
        "Deno.Kv can not be constructed: use Deno.openKv instead",
      );
    }
    this.#rid = rid;
    this.#isClosed = false;
  }

  atomic() {
    return new AtomicOperation(this.#rid);
  }

  commitVersionstamp(): symbol {
    return commitVersionstampSymbol;
  }

  async get(key: Deno.KvKey, opts?: { consistency?: Deno.KvConsistencyLevel }) {
    const { 0: entries }: [RawKvEntry[]] = await op_kv_snapshot_read(
      this.#rid,
      [[
        null,
        key,
        null,
        1,
        false,
        null,
      ]],
      opts?.consistency ?? "strong",
    );
    if (!entries.length) {
      return {
        key,
        value: null,
        versionstamp: null,
      };
    }
    return deserializeValue(entries[0]);
  }

  async getMany(
    keys: Deno.KvKey[],
    opts?: { consistency?: Deno.KvConsistencyLevel },
  ): Promise<Deno.KvEntry<unknown>[]> {
    const ranges: RawKvEntry[][] = await op_kv_snapshot_read(
      this.#rid,
      ArrayPrototypeMap(keys, (key: Deno.KvKey) => [
        null,
        key,
        null,
        1,
        false,
        null,
      ]),
      opts?.consistency ?? "strong",
    );
    return ArrayPrototypeMap(ranges, (entries: RawKvEntry[], i: number) => {
      if (!entries.length) {
        return {
          key: keys[i],
          value: null,
          versionstamp: null,
        };
      }
      return deserializeValue(entries[0]);
    });
  }

  async set(key: Deno.KvKey, value: unknown, options?: { expireIn?: number }) {
    const versionstamp = await doAtomicWriteInPlace(
      this.#rid,
      [],
      [[key, "set", serializeValue(value), options?.expireIn]],
      [],
    );
    if (versionstamp === null) throw new TypeError("Failed to set value");
    return { ok: true, versionstamp };
  }

  async delete(key: Deno.KvKey) {
    const result = await doAtomicWriteInPlace(
      this.#rid,
      [],
      [[key, "delete", null, undefined]],
      [],
    );
    if (!result) throw new TypeError("Failed to set value");
  }

  list(
    selector: Deno.KvListSelector,
    options: {
      limit?: number;
      batchSize?: number;
      cursor?: string;
      reverse?: boolean;
      consistency?: Deno.KvConsistencyLevel;
    } = { __proto__: null },
  ): KvListIterator {
    if (options.limit !== undefined && options.limit <= 0) {
      throw new Error(`Limit must be positive: received ${options.limit}`);
    }

    let batchSize = options.batchSize ?? (options.limit ?? 100);
    if (batchSize <= 0) throw new Error("batchSize must be positive");
    if (options.batchSize === undefined && batchSize > 500) batchSize = 500;

    return new KvListIterator({
      limit: options.limit,
      selector,
      cursor: options.cursor,
      reverse: options.reverse ?? false,
      consistency: options.consistency ?? "strong",
      batchSize,
      pullBatch: this.#pullBatch(batchSize),
    });
  }

  #pullBatch(batchSize: number): (
    selector: Deno.KvListSelector,
    cursor: string | undefined,
    reverse: boolean,
    consistency: Deno.KvConsistencyLevel,
  ) => Promise<Deno.KvEntry<unknown>[]> {
    return async (selector, cursor, reverse, consistency) => {
      const { 0: entries }: [RawKvEntry[]] = await op_kv_snapshot_read(
        this.#rid,
        [[
          ObjectHasOwn(selector, "prefix") ? selector.prefix : null,
          ObjectHasOwn(selector, "start") ? selector.start : null,
          ObjectHasOwn(selector, "end") ? selector.end : null,
          batchSize,
          reverse,
          cursor,
        ]],
        consistency,
      );

      return ArrayPrototypeMap(entries, deserializeValue);
    };
  }

  async enqueue(
    message: unknown,
    opts?: {
      delay?: number;
      keysIfUndelivered?: Deno.KvKey[];
      backoffSchedule?: number[];
    },
  ) {
    if (opts?.delay !== undefined) {
      validateQueueDelay(opts?.delay);
    }
    if (opts?.backoffSchedule !== undefined) {
      validateBackoffSchedule(opts?.backoffSchedule);
    }

    const versionstamp = await doAtomicWriteInPlace(
      this.#rid,
      [],
      [],
      [
        [
          core.serialize(message, { forStorage: true }),
          opts?.delay ?? 0,
          opts?.keysIfUndelivered ?? [],
          opts?.backoffSchedule ?? null,
        ],
      ],
    );
    if (versionstamp === null) throw new TypeError("Failed to enqueue value");
    return { ok: true, versionstamp };
  }

  async listenQueue(
    handler: (message: unknown) => Promise<void> | void,
  ): Promise<void> {
    if (this.#isClosed) {
      throw new Error("Queue already closed");
    }
    const finishMessageOps = new SafeMap<number, Promise<void>>();
    while (true) {
      // Wait for the next message.
      const next: { 0: Uint8Array; 1: number } =
        await op_kv_dequeue_next_message(
          this.#rid,
        );
      if (next === null) {
        break;
      }

      // Deserialize the payload.
      const { 0: payload, 1: handleId } = next;
      const deserializedPayload = core.deserialize(payload, {
        forStorage: true,
      });

      // Dispatch the payload.
      (async () => {
        let success = false;
        try {
          const result = handler(deserializedPayload);
          const _res = isPromise(result) ? (await result) : result;
          success = true;
        } catch (error) {
          import.meta.log("error", "Exception in queue handler", error);
        } finally {
          const promise: Promise<void> = op_kv_finish_dequeued_message(
            handleId,
            success,
          );
          finishMessageOps.set(handleId, promise);
          try {
            await promise;
          } finally {
            finishMessageOps.delete(handleId);
          }
        }
      })();
    }

    for (const { 1: promise } of new SafeMapIterator(finishMessageOps)) {
      await promise;
    }
    finishMessageOps.clear();
  }

  watch(keys: Deno.KvKey[], options = { __proto__: null }) {
    const raw = options.raw ?? false;
    const rid = op_kv_watch(this.#rid, keys);
    const lastEntries: (Deno.KvEntryMaybe<unknown> | undefined)[] = ArrayFrom(
      { length: keys.length },
    );
    return new ReadableStream({
      async pull(controller) {
        while (true) {
          let updates;
          try {
            updates = await op_kv_watch_next(rid);
          } catch (err) {
            core.tryClose(rid);
            controller.error(err);
            return;
          }
          if (updates === null) {
            core.tryClose(rid);
            controller.close();
            return;
          }
          let changed = false;
          for (let i = 0; i < keys.length; i++) {
            if (updates[i] === "unchanged") {
              if (lastEntries[i] === undefined) {
                throw new Error(
                  "'watch': invalid unchanged update (internal error)",
                );
              }
              continue;
            }
            if (
              lastEntries[i] !== undefined &&
              (updates[i]?.versionstamp ?? null) ===
                lastEntries[i]?.versionstamp
            ) {
              continue;
            }
            changed = true;
            if (updates[i] === null) {
              lastEntries[i] = {
                key: ArrayPrototypeSlice(keys[i]),
                value: null,
                versionstamp: null,
              };
            } else {
              lastEntries[i] = updates[i];
            }
          }
          if (!changed && !raw) continue; // no change
          const entries = ArrayPrototypeMap(
            lastEntries,
            (entry) =>
              entry.versionstamp === null
                ? { ...entry }
                : deserializeValue(entry),
          );
          controller.enqueue(entries);
          return;
        }
      },
      cancel() {
        core.tryClose(rid);
      },
    });
  }

  close() {
    core.close(this.#rid);
    this.#isClosed = true;
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
  }
}

class AtomicOperation {
  #rid: number;

  #checks: [Deno.KvKey, string | null][] = [];
  #mutations: [Deno.KvKey, string, RawValue | null, number | undefined][] = [];
  #enqueues: [Uint8Array, number, Deno.KvKey[], number[] | null][] = [];

  constructor(rid: number) {
    this.#rid = rid;
  }

  check(...checks: Deno.AtomicCheck[]): this {
    for (let i = 0; i < checks.length; ++i) {
      const check = checks[i];
      ArrayPrototypePush(this.#checks, [check.key, check.versionstamp]);
    }
    return this;
  }

  mutate(...mutations: Deno.KvMutation[]): this {
    for (let i = 0; i < mutations.length; ++i) {
      const mutation = mutations[i];
      const key = mutation.key;
      let type: string;
      let value: RawValue | null;
      let expireIn: number | undefined = undefined;
      switch (mutation.type) {
        case "delete":
          type = "delete";
          if (mutation.value) {
            throw new TypeError("Invalid mutation 'delete' with value");
          }
          break;
        case "set":
          if (typeof mutation.expireIn === "number") {
            expireIn = mutation.expireIn;
          }
          /* falls through */
        case "sum":
        case "min":
        case "max":
          type = mutation.type;
          if (!ObjectHasOwn(mutation, "value")) {
            throw new TypeError(`Invalid mutation '${type}' without value`);
          }
          value = serializeValue(mutation.value);
          break;
        default:
          throw new TypeError("Invalid mutation type");
      }
      ArrayPrototypePush(this.#mutations, [key, type, value, expireIn]);
    }
    return this;
  }

  sum(key: Deno.KvKey, n: bigint): this {
    ArrayPrototypePush(this.#mutations, [
      key,
      "sum",
      serializeValue(new KvU64(n)),
      undefined,
    ]);
    return this;
  }

  min(key: Deno.KvKey, n: bigint): this {
    ArrayPrototypePush(this.#mutations, [
      key,
      "min",
      serializeValue(new KvU64(n)),
      undefined,
    ]);
    return this;
  }

  max(key: Deno.KvKey, n: bigint): this {
    ArrayPrototypePush(this.#mutations, [
      key,
      "max",
      serializeValue(new KvU64(n)),
      undefined,
    ]);
    return this;
  }

  set(
    key: Deno.KvKey,
    value: unknown,
    options?: { expireIn?: number },
  ): this {
    ArrayPrototypePush(this.#mutations, [
      key,
      "set",
      serializeValue(value),
      options?.expireIn,
    ]);
    return this;
  }

  delete(key: Deno.KvKey): this {
    ArrayPrototypePush(this.#mutations, [key, "delete", null, undefined]);
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
    if (opts?.delay !== undefined) {
      validateQueueDelay(opts?.delay);
    }
    if (opts?.backoffSchedule !== undefined) {
      validateBackoffSchedule(opts?.backoffSchedule);
    }
    ArrayPrototypePush(this.#enqueues, [
      core.serialize(message, { forStorage: true }),
      opts?.delay ?? 0,
      opts?.keysIfUndelivered ?? [],
      opts?.backoffSchedule ?? null,
    ]);
    return this;
  }

  async commit(): Promise<Deno.KvCommitResult | Deno.KvCommitError> {
    const versionstamp = await doAtomicWriteInPlace(
      this.#rid,
      this.#checks,
      this.#mutations,
      this.#enqueues,
    );
    if (versionstamp === null) return { ok: false };
    return { ok: true, versionstamp };
  }

  then() {
    throw new TypeError(
      "'Deno.AtomicOperation' is not a promise: did you forget to call 'commit()'",
    );
  }
}

const MIN_U64 = BigInt("0");
const MAX_U64 = BigInt("0xffffffffffffffff");

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

  valueOf() {
    return this.value;
  }

  toString() {
    return BigIntPrototypeToString(this.value);
  }

  get [SymbolToStringTag]() {
    return "Deno.KvU64";
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return StringPrototypeReplace(
      inspect(Object(this.value), inspectOptions),
      "BigInt",
      "Deno.KvU64",
    );
  }
}

function deserializeValue(entry: RawKvEntry): Deno.KvEntry<unknown> {
  const { kind, value } = entry.value;
  switch (kind) {
    case "v8":
      return {
        ...entry,
        value: core.deserialize(value, { forStorage: true }),
      };
    case "bytes":
      return {
        ...entry,
        value,
      };
    case "u64":
      return {
        ...entry,
        value: new KvU64(value),
      };
    default:
      throw new TypeError("Invalid value type");
  }
}

function serializeValue(value: unknown): RawValue {
  if (TypedArrayPrototypeGetSymbolToStringTag(value) === "Uint8Array") {
    return {
      kind: "bytes",
      value,
    };
  } else if (ObjectPrototypeIsPrototypeOf(KvU64.prototype, value)) {
    return {
      kind: "u64",
      // deno-lint-ignore prefer-primordials
      value: value.valueOf(),
    };
  } else {
    return {
      kind: "v8",
      value: core.serialize(value, { forStorage: true }),
    };
  }
}

// This gets the %AsyncIteratorPrototype% object (which exists but is not a
// global). We extend the KvListIterator iterator from, so that we immediately
// support async iterator helpers once they land. The %AsyncIterator% does not
// yet actually exist however, so right now the AsyncIterator binding refers to
// %Object%. I know.
// Once AsyncIterator is a global, we can just use it (from primordials), rather
// than doing this here.
const AsyncIteratorPrototype = ObjectGetPrototypeOf(AsyncGeneratorPrototype);
const AsyncIterator = AsyncIteratorPrototype.constructor;

class KvListIterator extends AsyncIterator
  implements AsyncIterator<Deno.KvEntry<unknown>> {
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

  constructor(
    { limit, selector, cursor, reverse, consistency, batchSize, pullBatch }: {
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
    },
  ) {
    super();
    let prefix: Deno.KvKey | undefined;
    let start: Deno.KvKey | undefined;
    let end: Deno.KvKey | undefined;
    if (ObjectHasOwn(selector, "prefix") && selector.prefix !== undefined) {
      prefix = ObjectFreeze(ArrayPrototypeSlice(selector.prefix));
    }
    if (ObjectHasOwn(selector, "start") && selector.start !== undefined) {
      start = ObjectFreeze(ArrayPrototypeSlice(selector.start));
    }
    if (ObjectHasOwn(selector, "end") && selector.end !== undefined) {
      end = ObjectFreeze(ArrayPrototypeSlice(selector.end));
    }
    if (prefix) {
      if (start && end) {
        throw new TypeError(
          "Selector can not specify both 'start' and 'end' key when specifying 'prefix'",
        );
      }
      if (start) {
        this.#selector = { prefix, start };
      } else if (end) {
        this.#selector = { prefix, end };
      } else {
        this.#selector = { prefix };
      }
    } else {
      if (start && end) {
        this.#selector = { start, end };
      } else {
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
    // Fused or limit exceeded
    if (
      this.#done ||
      (this.#limit !== undefined && this.#count >= this.#limit)
    ) {
      return { done: true, value: undefined };
    }

    // Attempt to fill the buffer
    if (!this.#entries?.length && !this.#lastBatch) {
      const batch = await this.#pullBatch(
        this.#selector,
        this.#cursorGen ? this.#cursorGen() : undefined,
        this.#reverse,
        this.#consistency,
      );

      // Reverse the batch so we can pop from the end
      ArrayPrototypeReverse(batch);
      this.#entries = batch;

      // Last batch, do not attempt to pull more
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
      const selector = this.#selector;
      return encodeCursor([
        ObjectHasOwn(selector, "prefix") ? selector.prefix : null,
        ObjectHasOwn(selector, "start") ? selector.start : null,
        ObjectHasOwn(selector, "end") ? selector.end : null,
      ], entry.key);
    };
    this.#count++;
    return {
      done: false,
      value: entry,
    };
  }

  [SymbolAsyncIterator](): AsyncIterator<Deno.KvEntry<unknown>> {
    return this;
  }
}

async function doAtomicWriteInPlace(
  rid: number,
  checks: [Deno.KvKey, string | null][],
  mutations: [Deno.KvKey, string, RawValue | null, number | undefined][],
  enqueues: [Uint8Array, number, Deno.KvKey[], number[] | null][],
): Promise<string | null> {
  for (let i = 0; i < mutations.length; ++i) {
    const mutation = mutations[i];
    const key = mutation[0];
    if (
      key.length && mutation[1] === "set" &&
      key[key.length - 1] === commitVersionstampSymbol
    ) {
      mutation[0] = ArrayPrototypeSlice(key, 0, key.length - 1);
      mutation[1] = "setSuffixVersionstampedKey";
    }
  }

  return await op_kv_atomic_write(
    rid,
    checks,
    mutations,
    enqueues,
  );
}

export { AtomicOperation, Kv, KvListIterator, KvU64, openKv };
