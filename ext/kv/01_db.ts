// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-ignore internal api
const core = Deno.core;
const ops = core.ops;

const encodeKey: (key: Deno.KvKey) => Uint8Array = (x) =>
  ops.op_kv_encode_key(x);
const base64urlEncode: (data: Uint8Array) => string = (x) =>
  ops.op_crypto_base64url_encode(x);
const base64urlDecode: (data: string) => Uint8Array = (x) =>
  ops.op_crypto_base64url_decode(x);

async function openDatabase(path: string) {
  const rid = await core.opAsync("op_kv_database_open", path);
  return new Database(rid);
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

class Database {
  #rid: number;

  constructor(rid: number) {
    this.#rid = rid;
  }

  atomic() {
    return new AtomicOperation(this.#rid);
  }

  async get(key: Deno.KvKey, opts?: { consistency?: Deno.ConsistencyLevel }) {
    key = convertKey(key);
    const [entries]: [RawKvEntry[]] = await core.opAsync(
      "op_kv_snapshot_read",
      this.#rid,
      [[
        encodeKey(key),
        null,
        1,
        false,
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

  async set(key: Deno.KvKey, value: unknown) {
    key = convertKey(key);
    value = serializeValue(value);

    const checks: Deno.AtomicCheck[] = [];
    const mutations = [
      [key, "set", value],
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

  async delete(key: Deno.KvKey) {
    key = convertKey(key);

    const checks: Deno.AtomicCheck[] = [];
    const mutations = [
      [key, "delete", null],
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
      consistency?: Deno.ConsistencyLevel;
    } = {},
  ): KvListIterator {
    if (options.limit !== undefined && options.limit <= 0) {
      throw new Error("limit must be positive");
    }

    let batchSize = options.batchSize ?? (options.limit ?? 100);
    if (batchSize <= 0) throw new Error("batchSize must be positive");
    if (batchSize > 500) batchSize = 500;

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
    consistency: Deno.ConsistencyLevel,
  ) => Promise<Deno.KvEntry[]> {
    return async (selector, cursor, reverse, consistency) => {
      const { firstKey, lastKey } = decodeCursor(selector, cursor, reverse);

      const [entries]: [RawKvEntry[]] = await core.opAsync(
        "op_kv_snapshot_read",
        this.#rid,
        [[
          firstKey,
          lastKey,
          batchSize,
          reverse,
        ]],
        consistency,
      );

      return entries.map(deserializeValue);
    };
  }

  close() {
    core.close(this.#rid);
  }
}

class AtomicOperation {
  #rid: number;

  #checks: [Deno.KvKey, string | null][] = [];
  #mutations: [Deno.KvKey, string, RawValue | null][] = [];

  constructor(rid: number) {
    this.#rid = rid;
  }

  check(...checks: Deno.AtomicCheck[]): this {
    for (const check of checks) {
      this.#checks.push([convertKey(check.key), check.versionstamp]);
    }
    return this;
  }

  mutate(...mutations: Deno.KvMutation[]): this {
    for (const mutation of mutations) {
      const key = convertKey(mutation.key);
      let type: string;
      let value: RawValue | null;
      switch (mutation.type) {
        case "delete":
          type = "delete";
          value = null;
          break;
        case "set":
        case "sum":
        case "min":
        case "max":
          type = mutation.type;
          value = serializeValue(mutation.value);
          break;
        default:
          throw new TypeError("Invalid mutation type");
      }
      this.#mutations.push([key, type, value]);
    }
    return this;
  }

  set(key: Deno.KvKey, value: unknown): this {
    this.#mutations.push([convertKey(key), "set", serializeValue(value)]);
    return this;
  }

  delete(key: Deno.KvKey): this {
    this.#mutations.push([convertKey(key), "delete", null]);
    return this;
  }

  async commit(): Promise<boolean> {
    const result = await core.opAsync(
      "op_kv_atomic_write",
      this.#rid,
      this.#checks,
      this.#mutations,
      [], // TODO(@losfair): enqueue
    );
    return result;
  }

  then() {
    throw new TypeError(
      "`Deno.AtomicOperation` is not a promise. Did you forget to call `commit()`?",
    );
  }
}

const MIN_U64 = 0n;
const MAX_U64 = 0xffffffffffffffffn;

class KvU64 {
  #value: bigint;

  constructor(value: bigint) {
    if (typeof value !== "bigint") {
      throw new TypeError("value must be a bigint");
    }
    if (value < MIN_U64) {
      throw new TypeError("value must be a positive bigint");
    }
    if (value > MAX_U64) {
      throw new TypeError("value must be a 64-bit unsigned integer");
    }
    this.#value = value;
    Object.freeze(this);
  }

  get value(): bigint {
    return this.#value;
  }
}

function convertKey(key: Deno.KvKey | Deno.KvKeyPart): Deno.KvKey {
  if (Array.isArray(key)) {
    return key;
  } else {
    return [key as Deno.KvKeyPart];
  }
}

function deserializeValue(entry: RawKvEntry): Deno.KvEntry {
  const { kind, value } = entry.value;
  switch (kind) {
    case "v8":
      return {
        ...entry,
        value: core.deserialize(value),
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
  if (value instanceof Uint8Array) {
    return {
      kind: "bytes",
      value,
    };
  } else if (value instanceof KvU64) {
    return {
      kind: "u64",
      value: value.value,
    };
  } else {
    return {
      kind: "v8",
      value: core.serialize(value),
    };
  }
}

class KvListIterator {
  #selector: Deno.KvListSelector;
  #entries: Deno.KvEntry[] | null = null;
  #cursorGen: (() => string) | null = null;
  #done = false;
  #lastBatch = false;
  #pullBatch: (
    selector: Deno.KvListSelector,
    cursor: string | undefined,
    reverse: boolean,
    consistency: Deno.ConsistencyLevel,
  ) => Promise<Deno.KvEntry[]>;
  #limit: number | undefined;
  #count = 0;
  #reverse: boolean;
  #batchSize: number;
  #consistency: Deno.ConsistencyLevel;

  constructor(
    { limit, selector, cursor, reverse, consistency, batchSize, pullBatch }: {
      limit?: number;
      selector: Deno.KvListSelector;
      cursor?: string;
      reverse: boolean;
      batchSize: number;
      consistency: Deno.ConsistencyLevel;
      pullBatch: (
        selector: Deno.KvListSelector,
        cursor: string | undefined,
        reverse: boolean,
        consistency: Deno.ConsistencyLevel,
      ) => Promise<Deno.KvEntry[]>;
    },
  ) {
    this.#selector = Object.freeze(
      "prefix" in selector
        ? {
          prefix: Object.freeze([...selector.prefix]),
        }
        : {
          start: Object.freeze([...selector.start]),
          end: Object.freeze([...selector.end]),
        },
    );
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

  [Symbol.asyncIterator](): AsyncIterator<Deno.KvEntry> {
    return {
      next: async () => {
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

        this.#cursorGen = () =>
          encodeCursor(this.#selector, encodeKey(entry.key));
        this.#count++;
        return {
          done: false,
          value: entry,
        };
      },
    };
  }
}

function getCommonPrefixForBytes(a: Uint8Array, b: Uint8Array): Uint8Array {
  const minLen = Math.min(a.byteLength, b.byteLength);
  let maxCommonPrefixLength = minLen;
  for (let i = 0; i < minLen; i++) {
    if (a[i] !== b[i]) {
      maxCommonPrefixLength = i;
      break;
    }
  }

  return a.subarray(0, maxCommonPrefixLength);
}

function getCommonPrefixForSelector(selector: Deno.KvListSelector): Uint8Array {
  if ("prefix" in selector) {
    return encodeKey(selector.prefix);
  } else {
    return getCommonPrefixForBytes(
      encodeKey(selector.start),
      encodeKey(selector.end),
    );
  }
}

function encodeCursor(
  selector: Deno.KvListSelector,
  boundaryKey: Uint8Array,
): string {
  const commonPrefix = getCommonPrefixForSelector(selector);

  if (
    commonPrefix.byteLength > boundaryKey.byteLength ||
    commonPrefix.findIndex((x, i) => x !== boundaryKey[i]) !== -1
  ) {
    throw new Error("Invalid boundaryKey");
  }

  return base64urlEncode(boundaryKey.subarray(commonPrefix.byteLength));
}

function decodeCursor(
  selector: Deno.KvListSelector,
  cursor: string | undefined,
  reverse: boolean,
): { firstKey: Uint8Array; lastKey: Uint8Array } {
  const getRangeStartKey = () =>
    encodeKey(
      "start" in selector ? selector.start : selector.prefix,
    );

  const getRangeEndKey = () => {
    if ("prefix" in selector) {
      const prefix = encodeKey(selector.prefix);
      const lastKey = new Uint8Array(prefix.byteLength + 1);
      lastKey.set(prefix);
      lastKey[prefix.byteLength] = 0xff;
      return lastKey;
    } else {
      return encodeKey(selector.end);
    }
  };

  // If no cursor is provided, start from beginning
  if (!cursor) {
    return {
      firstKey: getRangeStartKey(),
      lastKey: getRangeEndKey(),
    };
  }

  const commonPrefix = getCommonPrefixForSelector(selector);
  const decodedCursor = base64urlDecode(cursor);

  let firstKey: Uint8Array;
  let lastKey: Uint8Array;

  if (reverse) {
    firstKey = getRangeStartKey();

    // Last key is exclusive - no need to append a zero byte
    lastKey = new Uint8Array(
      commonPrefix.byteLength + decodedCursor.byteLength,
    );
    lastKey.set(commonPrefix);
    lastKey.set(decodedCursor, commonPrefix.byteLength);
  } else {
    // append a zero byte - `${key}\0` immediately follows `${key}`
    firstKey = new Uint8Array(
      commonPrefix.byteLength + decodedCursor.byteLength + 1,
    );
    firstKey.set(commonPrefix);
    firstKey.set(decodedCursor, commonPrefix.byteLength);

    lastKey = getRangeEndKey();
  }

  // Defend against out-of-bounds reading
  if ("start" in selector) {
    const start = encodeKey(selector.start);
    if (compareBytes(firstKey, start) < 0) {
      throw new Error("cursor out of bounds");
    }
  }

  if ("end" in selector) {
    const end = encodeKey(selector.end);
    if (compareBytes(lastKey, end) > 0) {
      throw new Error("cursor out of bounds");
    }
  }

  return { firstKey, lastKey };
}

// Three-way comparison
function compareBytes(lhs: Uint8Array, rhs: Uint8Array): 1 | 0 | -1 {
  const minLen = Math.min(lhs.byteLength, rhs.byteLength);
  for (let i = 0; i < minLen; i++) {
    if (lhs[i] < rhs[i]) {
      return -1;
    } else if (lhs[i] > rhs[i]) {
      return 1;
    }
  }
  if (lhs.byteLength < rhs.byteLength) {
    return -1;
  } else if (lhs.byteLength > rhs.byteLength) {
    return 1;
  } else {
    return 0;
  }
}

export { Database, KvListIterator, KvU64, openDatabase };
