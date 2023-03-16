// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// DO NOT EDIT: generated from 01_db.ts

const core = Deno.core;
const ops = core.ops;
const encodeKey = (x) => ops.op_kv_encode_key(x);
const base64urlEncode = (x) => ops.op_crypto_base64url_encode(x);
const base64urlDecode = (x) => ops.op_crypto_base64url_decode(x);
async function openDatabase(path) {
  const rid = await core.opAsync("op_kv_database_open", path);
  return new Database(rid);
}
class Database {
  #rid;
  constructor(rid) {
    this.#rid = rid;
  }
  atomic() {
    return new AtomicOperation(this.#rid);
  }
  async get(key, opts) {
    key = convertKey(key);
    const [entries] = await core.opAsync("op_kv_snapshot_read", this.#rid, [[
      encodeKey(key),
      null,
      1,
      false,
    ]], opts?.consistency ?? "strong");
    if (!entries.length) {
      return {
        key,
        value: null,
        versionstamp: null,
      };
    }
    return deserializeValue(entries[0]);
  }
  async set(key, value) {
    key = convertKey(key);
    value = serializeValue(value);
    const checks = [];
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
    if (!result) {
      throw new TypeError("Failed to set value");
    }
  }
  async delete(key) {
    key = convertKey(key);
    const checks = [];
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
    if (!result) {
      throw new TypeError("Failed to set value");
    }
  }
  list(selector, options = {}) {
    if (options.limit !== void 0 && options.limit <= 0) {
      throw new Error("limit must be positive");
    }
    let batchSize = options.batchSize ?? (options.limit ?? 100);
    if (batchSize <= 0) {
      throw new Error("batchSize must be positive");
    }
    if (batchSize > 500) {
      batchSize = 500;
    }
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
  #pullBatch(batchSize) {
    return async (selector, cursor, reverse, consistency) => {
      const { firstKey, lastKey } = decodeCursor(selector, cursor, reverse);
      const [entries] = await core.opAsync("op_kv_snapshot_read", this.#rid, [[
        firstKey,
        lastKey,
        batchSize,
        reverse,
      ]], consistency);
      return entries.map(deserializeValue);
    };
  }
  close() {
    core.close(this.#rid);
  }
}
class AtomicOperation {
  #rid;
  #checks = [];
  #mutations = [];
  constructor(rid) {
    this.#rid = rid;
  }
  check(...checks) {
    for (const check of checks) {
      this.#checks.push([convertKey(check.key), check.versionstamp]);
    }
    return this;
  }
  mutate(...mutations) {
    for (const mutation of mutations) {
      const key = convertKey(mutation.key);
      let type;
      let value;
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
  set(key, value) {
    this.#mutations.push([convertKey(key), "set", serializeValue(value)]);
    return this;
  }
  delete(key) {
    this.#mutations.push([convertKey(key), "delete", null]);
    return this;
  }
  async commit() {
    const result = await core.opAsync(
      "op_kv_atomic_write",
      this.#rid,
      this.#checks,
      this.#mutations,
      [],
    );
    return result;
  }
  then() {
    throw new TypeError(
      "`Deno.AtomicOperation` is not a promise. Did you forget to call `commit()`?",
    );
  }
}
class KvU64 {
  constructor(value) {
    this.value = value;
    Object.freeze(this);
  }
}
function convertKey(key) {
  if (Array.isArray(key)) {
    return key;
  } else {
    return [key];
  }
}
function deserializeValue(entry) {
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
function serializeValue(value) {
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
  #selector;
  #entries = null;
  #cursorGen = null;
  #done = false;
  #lastBatch = false;
  #pullBatch;
  #limit;
  #count = 0;
  #reverse;
  #batchSize;
  #consistency;
  constructor(
    { limit, selector, cursor, reverse, consistency, batchSize, pullBatch },
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
  cursor() {
    if (this.#cursorGen === null) {
      throw new Error("Cannot get cursor before first iteration");
    }
    return this.#cursorGen();
  }
  [Symbol.asyncIterator]() {
    return {
      next: async () => {
        if (
          this.#done || this.#limit !== void 0 && this.#count >= this.#limit
        ) {
          return { done: true, value: void 0 };
        }
        if (!this.#entries?.length && !this.#lastBatch) {
          const batch = await this.#pullBatch(
            this.#selector,
            this.#cursorGen ? this.#cursorGen() : void 0,
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
          return { done: true, value: void 0 };
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
function getCommonPrefixForBytes(a, b) {
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
function getCommonPrefixForSelector(selector) {
  if ("prefix" in selector) {
    return encodeKey(selector.prefix);
  } else {
    return getCommonPrefixForBytes(
      encodeKey(selector.start),
      encodeKey(selector.end),
    );
  }
}
function encodeCursor(selector, boundaryKey) {
  const commonPrefix = getCommonPrefixForSelector(selector);
  if (
    commonPrefix.byteLength > boundaryKey.byteLength ||
    commonPrefix.findIndex((x, i) => x !== boundaryKey[i]) !== -1
  ) {
    throw new Error("Invalid boundaryKey");
  }
  return base64urlEncode(boundaryKey.subarray(commonPrefix.byteLength));
}
function decodeCursor(selector, cursor, reverse) {
  const getRangeStartKey = () =>
    encodeKey("start" in selector ? selector.start : selector.prefix);
  const getRangeEndKey = () => {
    if ("prefix" in selector) {
      const prefix = encodeKey(selector.prefix);
      const lastKey2 = new Uint8Array(prefix.byteLength + 1);
      lastKey2.set(prefix);
      lastKey2[prefix.byteLength] = 255;
      return lastKey2;
    } else {
      return encodeKey(selector.end);
    }
  };
  if (!cursor) {
    return {
      firstKey: getRangeStartKey(),
      lastKey: getRangeEndKey(),
    };
  }
  const commonPrefix = getCommonPrefixForSelector(selector);
  const decodedCursor = base64urlDecode(cursor);
  let firstKey;
  let lastKey;
  if (reverse) {
    firstKey = getRangeStartKey();
    lastKey = new Uint8Array(
      commonPrefix.byteLength + decodedCursor.byteLength,
    );
    lastKey.set(commonPrefix);
    lastKey.set(decodedCursor, commonPrefix.byteLength);
  } else {
    firstKey = new Uint8Array(
      commonPrefix.byteLength + decodedCursor.byteLength + 1,
    );
    firstKey.set(commonPrefix);
    firstKey.set(decodedCursor, commonPrefix.byteLength);
    lastKey = getRangeEndKey();
  }
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
function compareBytes(lhs, rhs) {
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
