// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-ignore internal api
const {
  AsyncGeneratorPrototype,
  BigIntPrototypeToString,
  ObjectFreeze,
  ObjectGetPrototypeOf,
  ObjectPrototypeIsPrototypeOf,
  StringPrototypeReplace,
  SymbolFor,
  SymbolToStringTag,
  Uint8ArrayPrototype,
} = globalThis.__bootstrap.primordials;
const core = Deno.core;
const ops = core.ops;

const encodeCursor: (
  selector: [Deno.KvKey | null, Deno.KvKey | null, Deno.KvKey | null],
  boundaryKey: Deno.KvKey,
) => string = (selector, boundaryKey) =>
  ops.op_kv_encode_cursor(selector, boundaryKey);

async function openKv(path: string) {
  const rid = await core.opAsync("op_kv_database_open", path);
  return new Kv(rid, kvSymbol);
}

const millisecondsInOneWeek = 7 * 24 * 60 * 60 * 1000;

function validateQueueDelay(delay: number) {
  if (delay < 0) {
    throw new TypeError("delay cannot be negative");
  }
  if (delay > millisecondsInOneWeek) {
    throw new TypeError("delay cannot be greater than one week");
  }
  if (isNaN(delay)) {
    throw new TypeError("delay cannot be NaN");
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

class Kv {
  #rid: number;
  #closed: boolean;

  constructor(rid: number = undefined, symbol: symbol = undefined) {
    if (kvSymbol !== symbol) {
      throw new TypeError(
        "Deno.Kv can not be constructed, use Deno.openKv instead.",
      );
    }
    this.#rid = rid;
    this.#closed = false;
  }

  atomic() {
    return new AtomicOperation(this.#rid);
  }

  async get(key: Deno.KvKey, opts?: { consistency?: Deno.KvConsistencyLevel }) {
    const [entries]: [RawKvEntry[]] = await core.opAsync(
      "op_kv_snapshot_read",
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
    const ranges: RawKvEntry[][] = await core.opAsync(
      "op_kv_snapshot_read",
      this.#rid,
      keys.map((key) => [
        null,
        key,
        null,
        1,
        false,
        null,
      ]),
      opts?.consistency ?? "strong",
    );
    return ranges.map((entries, i) => {
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
    value = serializeValue(value);

    const checks: Deno.AtomicCheck[] = [];
    const expireAt = typeof options?.expireIn === "number"
      ? Date.now() + options.expireIn
      : undefined;
    const mutations = [
      [key, "set", value, expireAt],
    ];

    const versionstamp = await core.opAsync(
      "op_kv_atomic_write",
      this.#rid,
      checks,
      mutations,
      [],
    );
    if (versionstamp === null) throw new TypeError("Failed to set value");
    return { ok: true, versionstamp };
  }

  async delete(key: Deno.KvKey) {
    const checks: Deno.AtomicCheck[] = [];
    const mutations = [
      [key, "delete", null, undefined],
    ];

    const result = await core.opAsync(
      "op_kv_atomic_write",
      this.#rid,
      checks,
      mutations,
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
    } = {},
  ): KvListIterator {
    if (options.limit !== undefined && options.limit <= 0) {
      throw new Error("limit must be positive");
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
      const [entries]: [RawKvEntry[]] = await core.opAsync(
        "op_kv_snapshot_read",
        this.#rid,
        [[
          "prefix" in selector ? selector.prefix : null,
          "start" in selector ? selector.start : null,
          "end" in selector ? selector.end : null,
          batchSize,
          reverse,
          cursor,
        ]],
        consistency,
      );

      return entries.map(deserializeValue);
    };
  }

  async enqueue(
    message: unknown,
    opts?: { delay?: number; keysIfUndelivered?: Deno.KvKey[] },
  ) {
    if (opts?.delay !== undefined) {
      validateQueueDelay(opts?.delay);
    }

    const enqueues = [
      [
        core.serialize(message, { forStorage: true }),
        opts?.delay ?? 0,
        opts?.keysIfUndelivered ?? [],
        null,
      ],
    ];

    const versionstamp = await core.opAsync(
      "op_kv_atomic_write",
      this.#rid,
      [],
      [],
      enqueues,
    );
    if (versionstamp === null) throw new TypeError("Failed to enqueue value");
    return { ok: true, versionstamp };
  }

  async listenQueue(
    handler: (message: unknown) => Promise<void> | void,
  ): Promise<void> {
    const finishMessageOps = new Map<number, Promise<void>>();
    while (!this.#closed) {
      // Wait for the next message.
      let next: { 0: Uint8Array; 1: number };
      try {
        next = await core.opAsync(
          "op_kv_dequeue_next_message",
          this.#rid,
        );
      } catch (error) {
        if (this.#closed) {
          break;
        } else {
          throw error;
        }
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
          const _res = result instanceof Promise ? (await result) : result;
          success = true;
        } catch (error) {
          console.error("Exception in queue handler", error);
        } finally {
          if (this.#closed) {
            core.close(handleId);
          } else {
            const promise: Promise<void> = core.opAsync(
              "op_kv_finish_dequeued_message",
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
        }
      })();
    }

    for (const promise of finishMessageOps.values()) {
      await promise;
    }
    finishMessageOps.clear();
  }

  close() {
    core.close(this.#rid);
    this.#closed = true;
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
    for (const check of checks) {
      this.#checks.push([check.key, check.versionstamp]);
    }
    return this;
  }

  mutate(...mutations: Deno.KvMutation[]): this {
    for (const mutation of mutations) {
      const key = mutation.key;
      let type: string;
      let value: RawValue | null;
      let expireAt: number | undefined = undefined;
      switch (mutation.type) {
        case "delete":
          type = "delete";
          if (mutation.value) {
            throw new TypeError("invalid mutation 'delete' with value");
          }
          break;
        case "set":
          if (typeof mutation.expireIn === "number") {
            expireAt = Date.now() + mutation.expireIn;
          }
          /* falls through */
        case "sum":
        case "min":
        case "max":
          type = mutation.type;
          if (!("value" in mutation)) {
            throw new TypeError(`invalid mutation '${type}' without value`);
          }
          value = serializeValue(mutation.value);
          break;
        default:
          throw new TypeError("Invalid mutation type");
      }
      this.#mutations.push([key, type, value, expireAt]);
    }
    return this;
  }

  sum(key: Deno.KvKey, n: bigint): this {
    this.#mutations.push([key, "sum", serializeValue(new KvU64(n)), undefined]);
    return this;
  }

  min(key: Deno.KvKey, n: bigint): this {
    this.#mutations.push([key, "min", serializeValue(new KvU64(n)), undefined]);
    return this;
  }

  max(key: Deno.KvKey, n: bigint): this {
    this.#mutations.push([key, "max", serializeValue(new KvU64(n)), undefined]);
    return this;
  }

  set(
    key: Deno.KvKey,
    value: unknown,
    options?: { expireIn?: number },
  ): this {
    const expireAt = typeof options?.expireIn === "number"
      ? Date.now() + options.expireIn
      : undefined;
    this.#mutations.push([key, "set", serializeValue(value), expireAt]);
    return this;
  }

  delete(key: Deno.KvKey): this {
    this.#mutations.push([key, "delete", null, undefined]);
    return this;
  }

  enqueue(
    message: unknown,
    opts?: { delay?: number; keysIfUndelivered?: Deno.KvKey[] },
  ): this {
    if (opts?.delay !== undefined) {
      validateQueueDelay(opts?.delay);
    }
    this.#enqueues.push([
      core.serialize(message, { forStorage: true }),
      opts?.delay ?? 0,
      opts?.keysIfUndelivered ?? [],
      null,
    ]);
    return this;
  }

  async commit(): Promise<Deno.KvCommitResult | Deno.KvCommitError> {
    const versionstamp = await core.opAsync(
      "op_kv_atomic_write",
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
      "`Deno.AtomicOperation` is not a promise. Did you forget to call `commit()`?",
    );
  }
}

const MIN_U64 = BigInt("0");
const MAX_U64 = BigInt("0xffffffffffffffff");

class KvU64 {
  value: bigint;

  constructor(value: bigint) {
    if (typeof value !== "bigint") {
      throw new TypeError("value must be a bigint");
    }
    if (value < MIN_U64) {
      throw new RangeError("value must be a positive bigint");
    }
    if (value > MAX_U64) {
      throw new RangeError("value must fit in a 64-bit unsigned integer");
    }
    this.value = value;
    Object.freeze(this);
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
  if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, value)) {
    return {
      kind: "bytes",
      value,
    };
  } else if (ObjectPrototypeIsPrototypeOf(KvU64.prototype, value)) {
    return {
      kind: "u64",
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
    if ("prefix" in selector && selector.prefix !== undefined) {
      prefix = ObjectFreeze([...selector.prefix]);
    }
    if ("start" in selector && selector.start !== undefined) {
      start = ObjectFreeze([...selector.start]);
    }
    if ("end" in selector && selector.end !== undefined) {
      end = ObjectFreeze([...selector.end]);
    }
    if (prefix) {
      if (start && end) {
        throw new TypeError(
          "Selector can not specify both 'start' and 'end' key when specifying 'prefix'.",
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
          "Selector must specify either 'prefix' or both 'start' and 'end' key.",
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
      batch.reverse();
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
        "prefix" in selector ? selector.prefix : null,
        "start" in selector ? selector.start : null,
        "end" in selector ? selector.end : null,
      ], entry.key);
    };
    this.#count++;
    return {
      done: false,
      value: entry,
    };
  }

  [Symbol.asyncIterator](): AsyncIterator<Deno.KvEntry<unknown>> {
    return this;
  }
}

export { AtomicOperation, Kv, KvListIterator, KvU64, openKv };
