// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  AssertionError,
  assertNotEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";
import { assertType, IsExact } from "@std/testing/types";

const sleep = (time: number) => new Promise((r) => setTimeout(r, time));

let isCI: boolean;
try {
  isCI = Deno.env.get("CI") !== undefined;
} catch {
  isCI = true;
}

// Defined in test_util/src/lib.rs
Deno.env.set("DENO_KV_ACCESS_TOKEN", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

Deno.test({
  name: "openKv :memory: no permissions",
  permissions: {},
  async fn() {
    const db = await Deno.openKv(":memory:");
    await db.close();
  },
});

Deno.test({
  name: "openKv invalid filenames",
  permissions: {},
  async fn() {
    await assertRejects(
      async () => await Deno.openKv(""),
      TypeError,
      "Filename cannot be empty",
    );
    await assertRejects(
      async () => await Deno.openKv(":foo"),
      TypeError,
      "Filename cannot start with ':' unless prefixed with './'",
    );
  },
});

function dbTest(name: string, fn: (db: Deno.Kv) => Promise<void> | void) {
  Deno.test({
    name,
    // https://github.com/denoland/deno/issues/18363
    ignore: Deno.build.os === "darwin" && isCI,
    async fn() {
      const db: Deno.Kv = await Deno.openKv(":memory:");
      try {
        await fn(db);
      } finally {
        db.close();
      }
    },
  });
}

function queueTest(name: string, fn: (db: Deno.Kv) => Promise<void>) {
  Deno.test({
    name,
    // https://github.com/denoland/deno/issues/18363
    ignore: Deno.build.os === "darwin" && isCI,
    async fn() {
      const db: Deno.Kv = await Deno.openKv(":memory:");
      await fn(db);
    },
  });
}

const ZERO_VERSIONSTAMP = "00000000000000000000";

dbTest("basic read-write-delete and versionstamps", async (db) => {
  const result1 = await db.get(["a"]);
  assertEquals(result1.key, ["a"]);
  assertEquals(result1.value, null);
  assertEquals(result1.versionstamp, null);

  const setRes = await db.set(["a"], "b");
  assert(setRes.ok);
  assert(setRes.versionstamp > ZERO_VERSIONSTAMP);
  const result2 = await db.get(["a"]);
  assertEquals(result2.key, ["a"]);
  assertEquals(result2.value, "b");
  assertEquals(result2.versionstamp, setRes.versionstamp);

  const setRes2 = await db.set(["a"], "c");
  assert(setRes2.ok);
  assert(setRes2.versionstamp > setRes.versionstamp);
  const result3 = await db.get(["a"]);
  assertEquals(result3.key, ["a"]);
  assertEquals(result3.value, "c");
  assertEquals(result3.versionstamp, setRes2.versionstamp);

  await db.delete(["a"]);
  const result4 = await db.get(["a"]);
  assertEquals(result4.key, ["a"]);
  assertEquals(result4.value, null);
  assertEquals(result4.versionstamp, null);
});

const VALUE_CASES = [
  { name: "string", value: "hello" },
  { name: "number", value: 42 },
  { name: "bigint", value: 42n },
  { name: "boolean", value: true },
  { name: "null", value: null },
  { name: "undefined", value: undefined },
  { name: "Date", value: new Date(0) },
  { name: "Uint8Array", value: new Uint8Array([1, 2, 3]) },
  { name: "ArrayBuffer", value: new ArrayBuffer(3) },
  { name: "array", value: [1, 2, 3] },
  { name: "object", value: { a: 1, b: 2 } },
  { name: "nested array", value: [[1, 2], [3, 4]] },
  { name: "nested object", value: { a: { b: 1 } } },
];

for (const { name, value } of VALUE_CASES) {
  dbTest(`set and get ${name} value`, async (db) => {
    await db.set(["a"], value);
    const result = await db.get(["a"]);
    assertEquals(result.key, ["a"]);
    assertEquals(result.value, value);
  });
}

dbTest("set and get recursive object", async (db) => {
  // deno-lint-ignore no-explicit-any
  const value: any = { a: undefined };
  value.a = value;
  await db.set(["a"], value);
  const result = await db.get(["a"]);
  assertEquals(result.key, ["a"]);
  // deno-lint-ignore no-explicit-any
  const resultValue: any = result.value;
  assert(resultValue.a === resultValue);
});

// invalid values (as per structured clone algorithm with _for storage_, NOT JSON)
const INVALID_VALUE_CASES = [
  { name: "function", value: () => {} },
  { name: "symbol", value: Symbol() },
  { name: "WeakMap", value: new WeakMap() },
  { name: "WeakSet", value: new WeakSet() },
  {
    name: "WebAssembly.Module",
    value: new WebAssembly.Module(
      new Uint8Array([0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00]),
    ),
  },
  {
    name: "SharedArrayBuffer",
    value: new SharedArrayBuffer(3),
  },
];

for (const { name, value } of INVALID_VALUE_CASES) {
  dbTest(`set and get ${name} value (invalid)`, async (db) => {
    await assertRejects(
      async () => await db.set(["a"], value),
      Error,
    );
    const res = await db.get(["a"]);
    assertEquals(res.key, ["a"]);
    assertEquals(res.value, null);
  });
}

const keys = [
  ["a"],
  ["a", "b"],
  ["a", "b", "c"],
  [1],
  ["a", 1],
  ["a", 1, "b"],
  [1n],
  ["a", 1n],
  ["a", 1n, "b"],
  [true],
  ["a", true],
  ["a", true, "b"],
  [new Uint8Array([1, 2, 3])],
  ["a", new Uint8Array([1, 2, 3])],
  ["a", new Uint8Array([1, 2, 3]), "b"],
  [1, 1n, true, new Uint8Array([1, 2, 3]), "a"],
];

for (const key of keys) {
  dbTest(`set and get ${Deno.inspect(key)} key`, async (db) => {
    await db.set(key, "b");
    const result = await db.get(key);
    assertEquals(result.key, key);
    assertEquals(result.value, "b");
  });
}

const INVALID_KEYS = [
  [null],
  [undefined],
  [],
  [{}],
  [new Date()],
  [new ArrayBuffer(3)],
  [new Uint8Array([1, 2, 3]).buffer],
  [["a", "b"]],
];

for (const key of INVALID_KEYS) {
  dbTest(`set and get invalid key ${Deno.inspect(key)}`, async (db) => {
    await assertRejects(
      async () => {
        // @ts-ignore - we are testing invalid keys
        await db.set(key, "b");
      },
      Error,
    );
  });
}

dbTest("compare and mutate", async (db) => {
  await db.set(["t"], "1");

  const currentValue = await db.get(["t"]);
  assert(currentValue.versionstamp);
  assert(currentValue.versionstamp > ZERO_VERSIONSTAMP);

  let res = await db.atomic()
    .check({ key: ["t"], versionstamp: currentValue.versionstamp })
    .set(currentValue.key, "2")
    .commit();
  assert(res.ok);
  assert(res.versionstamp > currentValue.versionstamp);

  const newValue = await db.get(["t"]);
  assertEquals(newValue.versionstamp, res.versionstamp);
  assertEquals(newValue.value, "2");

  res = await db.atomic()
    .check({ key: ["t"], versionstamp: currentValue.versionstamp })
    .set(currentValue.key, "3")
    .commit();
  assert(!res.ok);

  const newValue2 = await db.get(["t"]);
  assertEquals(newValue2.versionstamp, newValue.versionstamp);
  assertEquals(newValue2.value, "2");
});

dbTest("compare and mutate not exists", async (db) => {
  let res = await db.atomic()
    .check({ key: ["t"], versionstamp: null })
    .set(["t"], "1")
    .commit();
  assert(res.ok);
  assert(res.versionstamp > ZERO_VERSIONSTAMP);

  const newValue = await db.get(["t"]);
  assertEquals(newValue.versionstamp, res.versionstamp);
  assertEquals(newValue.value, "1");

  res = await db.atomic()
    .check({ key: ["t"], versionstamp: null })
    .set(["t"], "2")
    .commit();
  assert(!res.ok);
});

dbTest("atomic mutation helper (sum)", async (db) => {
  await db.set(["t"], new Deno.KvU64(42n));
  assertEquals((await db.get(["t"])).value, new Deno.KvU64(42n));

  await db.atomic().sum(["t"], 1n).commit();
  assertEquals((await db.get(["t"])).value, new Deno.KvU64(43n));
});

dbTest("atomic mutation helper (min)", async (db) => {
  await db.set(["t"], new Deno.KvU64(42n));
  assertEquals((await db.get(["t"])).value, new Deno.KvU64(42n));

  await db.atomic().min(["t"], 1n).commit();
  assertEquals((await db.get(["t"])).value, new Deno.KvU64(1n));

  await db.atomic().min(["t"], 2n).commit();
  assertEquals((await db.get(["t"])).value, new Deno.KvU64(1n));
});

dbTest("atomic mutation helper (max)", async (db) => {
  await db.set(["t"], new Deno.KvU64(42n));
  assertEquals((await db.get(["t"])).value, new Deno.KvU64(42n));

  await db.atomic().max(["t"], 41n).commit();
  assertEquals((await db.get(["t"])).value, new Deno.KvU64(42n));

  await db.atomic().max(["t"], 43n).commit();
  assertEquals((await db.get(["t"])).value, new Deno.KvU64(43n));
});

dbTest("compare multiple and mutate", async (db) => {
  const setRes1 = await db.set(["t1"], "1");
  const setRes2 = await db.set(["t2"], "2");
  assert(setRes1.ok);
  assert(setRes1.versionstamp > ZERO_VERSIONSTAMP);
  assert(setRes2.ok);
  assert(setRes2.versionstamp > ZERO_VERSIONSTAMP);

  const currentValue1 = await db.get(["t1"]);
  assertEquals(currentValue1.versionstamp, setRes1.versionstamp);
  const currentValue2 = await db.get(["t2"]);
  assertEquals(currentValue2.versionstamp, setRes2.versionstamp);

  const res = await db.atomic()
    .check({ key: ["t1"], versionstamp: currentValue1.versionstamp })
    .check({ key: ["t2"], versionstamp: currentValue2.versionstamp })
    .set(currentValue1.key, "3")
    .set(currentValue2.key, "4")
    .commit();
  assert(res.ok);
  assert(res.versionstamp > setRes2.versionstamp);

  const newValue1 = await db.get(["t1"]);
  assertEquals(newValue1.versionstamp, res.versionstamp);
  assertEquals(newValue1.value, "3");
  const newValue2 = await db.get(["t2"]);
  assertEquals(newValue2.versionstamp, res.versionstamp);
  assertEquals(newValue2.value, "4");

  // just one of the two checks failed
  const res2 = await db.atomic()
    .check({ key: ["t1"], versionstamp: newValue1.versionstamp })
    .check({ key: ["t2"], versionstamp: null })
    .set(newValue1.key, "5")
    .set(newValue2.key, "6")
    .commit();
  assert(!res2.ok);

  const newValue3 = await db.get(["t1"]);
  assertEquals(newValue3.versionstamp, res.versionstamp);
  assertEquals(newValue3.value, "3");
  const newValue4 = await db.get(["t2"]);
  assertEquals(newValue4.versionstamp, res.versionstamp);
  assertEquals(newValue4.value, "4");
});

dbTest("atomic mutation ordering (set before delete)", async (db) => {
  await db.set(["a"], "1");
  const res = await db.atomic()
    .set(["a"], "2")
    .delete(["a"])
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, null);
});

dbTest("atomic mutation ordering (delete before set)", async (db) => {
  await db.set(["a"], "1");
  const res = await db.atomic()
    .delete(["a"])
    .set(["a"], "2")
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, "2");
});

dbTest("atomic mutation type=set", async (db) => {
  const res = await db.atomic()
    .mutate({ key: ["a"], value: "1", type: "set" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, "1");
});

dbTest("atomic mutation type=set overwrite", async (db) => {
  await db.set(["a"], "1");
  const res = await db.atomic()
    .mutate({ key: ["a"], value: "2", type: "set" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, "2");
});

dbTest("atomic mutation type=delete", async (db) => {
  await db.set(["a"], "1");
  const res = await db.atomic()
    .mutate({ key: ["a"], type: "delete" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, null);
});

dbTest("atomic mutation type=delete no exists", async (db) => {
  const res = await db.atomic()
    .mutate({ key: ["a"], type: "delete" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, null);
});

dbTest("atomic mutation type=sum", async (db) => {
  await db.set(["a"], new Deno.KvU64(10n));
  const res = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(1n), type: "sum" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, new Deno.KvU64(11n));
});

dbTest("atomic mutation type=sum no exists", async (db) => {
  const res = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(1n), type: "sum" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assert(result.value);
  assertEquals(result.value, new Deno.KvU64(1n));
});

dbTest("atomic mutation type=sum wrap around", async (db) => {
  await db.set(["a"], new Deno.KvU64(0xffffffffffffffffn));
  const res = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(10n), type: "sum" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, new Deno.KvU64(9n));

  const res2 = await db.atomic()
    .mutate({
      key: ["a"],
      value: new Deno.KvU64(0xffffffffffffffffn),
      type: "sum",
    })
    .commit();
  assert(res2);
  const result2 = await db.get(["a"]);
  assertEquals(result2.value, new Deno.KvU64(8n));
});

dbTest("atomic mutation type=sum wrong type in db", async (db) => {
  await db.set(["a"], 1);
  await assertRejects(
    async () => {
      await db.atomic()
        .mutate({ key: ["a"], value: new Deno.KvU64(1n), type: "sum" })
        .commit();
    },
    TypeError,
    "Failed to perform 'sum' mutation on a non-U64 value in the database",
  );
});

dbTest("atomic mutation type=sum wrong type in mutation", async (db) => {
  await db.set(["a"], new Deno.KvU64(1n));
  await assertRejects(
    async () => {
      await db.atomic()
        // @ts-expect-error wrong type is intentional
        .mutate({ key: ["a"], value: 1, type: "sum" })
        .commit();
    },
    TypeError,
    "Cannot sum KvU64 with Number",
  );
});

dbTest("atomic mutation type=min", async (db) => {
  await db.set(["a"], new Deno.KvU64(10n));
  const res = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(5n), type: "min" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, new Deno.KvU64(5n));

  const res2 = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(15n), type: "min" })
    .commit();
  assert(res2);
  const result2 = await db.get(["a"]);
  assertEquals(result2.value, new Deno.KvU64(5n));
});

dbTest("atomic mutation type=min no exists", async (db) => {
  const res = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(1n), type: "min" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assert(result.value);
  assertEquals(result.value, new Deno.KvU64(1n));
});

dbTest("atomic mutation type=min wrong type in db", async (db) => {
  await db.set(["a"], 1);
  await assertRejects(
    async () => {
      await db.atomic()
        .mutate({ key: ["a"], value: new Deno.KvU64(1n), type: "min" })
        .commit();
    },
    TypeError,
    "Failed to perform 'min' mutation on a non-U64 value in the database",
  );
});

dbTest("atomic mutation type=min wrong type in mutation", async (db) => {
  await db.set(["a"], new Deno.KvU64(1n));
  await assertRejects(
    async () => {
      await db.atomic()
        // @ts-expect-error wrong type is intentional
        .mutate({ key: ["a"], value: 1, type: "min" })
        .commit();
    },
    TypeError,
    "Failed to perform 'min' mutation on a non-U64 operand",
  );
});

dbTest("atomic mutation type=max", async (db) => {
  await db.set(["a"], new Deno.KvU64(10n));
  const res = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(5n), type: "max" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assertEquals(result.value, new Deno.KvU64(10n));

  const res2 = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(15n), type: "max" })
    .commit();
  assert(res2);
  const result2 = await db.get(["a"]);
  assertEquals(result2.value, new Deno.KvU64(15n));
});

dbTest("atomic mutation type=max no exists", async (db) => {
  const res = await db.atomic()
    .mutate({ key: ["a"], value: new Deno.KvU64(1n), type: "max" })
    .commit();
  assert(res.ok);
  const result = await db.get(["a"]);
  assert(result.value);
  assertEquals(result.value, new Deno.KvU64(1n));
});

dbTest("atomic mutation type=max wrong type in db", async (db) => {
  await db.set(["a"], 1);
  await assertRejects(
    async () => {
      await db.atomic()
        .mutate({ key: ["a"], value: new Deno.KvU64(1n), type: "max" })
        .commit();
    },
    TypeError,
    "Failed to perform 'max' mutation on a non-U64 value in the database",
  );
});

dbTest("atomic mutation type=max wrong type in mutation", async (db) => {
  await db.set(["a"], new Deno.KvU64(1n));
  await assertRejects(
    async () => {
      await db.atomic()
        // @ts-expect-error wrong type is intentional
        .mutate({ key: ["a"], value: 1, type: "max" })
        .commit();
    },
    TypeError,
    "Failed to perform 'max' mutation on a non-U64 operand",
  );
});

Deno.test("KvU64 comparison", () => {
  const a = new Deno.KvU64(1n);
  const b = new Deno.KvU64(1n);
  assertEquals(a, b);
  assertThrows(() => {
    assertEquals(a, new Deno.KvU64(2n));
  }, AssertionError);
});

Deno.test("KvU64 overflow", () => {
  assertThrows(() => {
    new Deno.KvU64(2n ** 64n);
  }, RangeError);
});

Deno.test("KvU64 underflow", () => {
  assertThrows(() => {
    new Deno.KvU64(-1n);
  }, RangeError);
});

Deno.test("KvU64 unbox", () => {
  const a = new Deno.KvU64(1n);
  assertEquals(a.value, 1n);
});

Deno.test("KvU64 unbox with valueOf", () => {
  const a = new Deno.KvU64(1n);
  assertEquals(a.valueOf(), 1n);
});

Deno.test("KvU64 auto-unbox", () => {
  const a = new Deno.KvU64(1n);
  assertEquals(a as unknown as bigint + 1n, 2n);
});

Deno.test("KvU64 toString", () => {
  const a = new Deno.KvU64(1n);
  assertEquals(a.toString(), "1");
});

Deno.test("KvU64 inspect", () => {
  const a = new Deno.KvU64(1n);
  assertEquals(Deno.inspect(a), "[Deno.KvU64: 1n]");
});

async function collect<T>(
  iter: Deno.KvListIterator<T>,
): Promise<Deno.KvEntry<T>[]> {
  const entries: Deno.KvEntry<T>[] = [];
  for await (const entry of iter) {
    entries.push(entry);
  }
  return entries;
}

async function setupData(db: Deno.Kv): Promise<string> {
  const res = await db.atomic()
    .set(["a"], -1)
    .set(["a", "a"], 0)
    .set(["a", "b"], 1)
    .set(["a", "c"], 2)
    .set(["a", "d"], 3)
    .set(["a", "e"], 4)
    .set(["b"], 99)
    .set(["b", "a"], 100)
    .commit();
  assert(res.ok);
  return res.versionstamp;
}

dbTest("get many", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await db.getMany([["b", "a"], ["a"], ["c"]]);
  assertEquals(entries, [
    { key: ["b", "a"], value: 100, versionstamp },
    { key: ["a"], value: -1, versionstamp },
    { key: ["c"], value: null, versionstamp: null },
  ]);
});

dbTest("list prefix", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(db.list({ prefix: ["a"] }));
  assertEquals(entries, [
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "e"], value: 4, versionstamp },
  ]);
});

dbTest("list prefix empty", async (db) => {
  await setupData(db);
  const entries = await collect(db.list({ prefix: ["c"] }));
  assertEquals(entries.length, 0);

  const entries2 = await collect(db.list({ prefix: ["a", "f"] }));
  assertEquals(entries2.length, 0);
});

dbTest("list prefix with start", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(db.list({ prefix: ["a"], start: ["a", "c"] }));
  assertEquals(entries, [
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "e"], value: 4, versionstamp },
  ]);
});

dbTest("list prefix with start empty", async (db) => {
  await setupData(db);
  const entries = await collect(db.list({ prefix: ["a"], start: ["a", "f"] }));
  assertEquals(entries.length, 0);
});

dbTest("list prefix with start equal to prefix", async (db) => {
  await setupData(db);
  await assertRejects(
    async () => await collect(db.list({ prefix: ["a"], start: ["a"] })),
    TypeError,
    "Start key is not in the keyspace defined by prefix",
  );
});

dbTest("list prefix with start out of bounds", async (db) => {
  await setupData(db);
  await assertRejects(
    async () => await collect(db.list({ prefix: ["b"], start: ["a"] })),
    TypeError,
    "Start key is not in the keyspace defined by prefix",
  );
});

dbTest("list prefix with end", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(db.list({ prefix: ["a"], end: ["a", "c"] }));
  assertEquals(entries, [
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
  ]);
});

dbTest("list prefix with end empty", async (db) => {
  await setupData(db);
  const entries = await collect(db.list({ prefix: ["a"], end: ["a", "a"] }));
  assertEquals(entries.length, 0);
});

dbTest("list prefix with end equal to prefix", async (db) => {
  await setupData(db);
  await assertRejects(
    async () => await collect(db.list({ prefix: ["a"], end: ["a"] })),
    TypeError,
    "End key is not in the keyspace defined by prefix",
  );
});

dbTest("list prefix with end out of bounds", async (db) => {
  await setupData(db);
  await assertRejects(
    async () => await collect(db.list({ prefix: ["a"], end: ["b"] })),
    TypeError,
    "End key is not in the keyspace defined by prefix",
  );
});

dbTest("list prefix with empty prefix", async (db) => {
  const res = await db.set(["a"], 1);
  const entries = await collect(db.list({ prefix: [] }));
  assertEquals(entries, [
    { key: ["a"], value: 1, versionstamp: res.versionstamp },
  ]);
});

dbTest("list prefix reverse", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(db.list({ prefix: ["a"] }, { reverse: true }));
  assertEquals(entries, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "a"], value: 0, versionstamp },
  ]);
});

dbTest("list prefix reverse with start", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(
    db.list({ prefix: ["a"], start: ["a", "c"] }, { reverse: true }),
  );
  assertEquals(entries, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
  ]);
});

dbTest("list prefix reverse with start empty", async (db) => {
  await setupData(db);
  const entries = await collect(
    db.list({ prefix: ["a"], start: ["a", "f"] }, { reverse: true }),
  );
  assertEquals(entries.length, 0);
});

dbTest("list prefix reverse with end", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(
    db.list({ prefix: ["a"], end: ["a", "c"] }, { reverse: true }),
  );
  assertEquals(entries, [
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "a"], value: 0, versionstamp },
  ]);
});

dbTest("list prefix reverse with end empty", async (db) => {
  await setupData(db);
  const entries = await collect(
    db.list({ prefix: ["a"], end: ["a", "a"] }, { reverse: true }),
  );
  assertEquals(entries.length, 0);
});

dbTest("list prefix limit", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(db.list({ prefix: ["a"] }, { limit: 2 }));
  assertEquals(entries, [
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
  ]);
});

dbTest("list prefix limit reverse", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(
    db.list({ prefix: ["a"] }, { limit: 2, reverse: true }),
  );
  assertEquals(entries, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
  ]);
});

dbTest("list prefix with small batch size", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(db.list({ prefix: ["a"] }, { batchSize: 2 }));
  assertEquals(entries, [
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "e"], value: 4, versionstamp },
  ]);
});

dbTest("list prefix with small batch size reverse", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(
    db.list({ prefix: ["a"] }, { batchSize: 2, reverse: true }),
  );
  assertEquals(entries, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "a"], value: 0, versionstamp },
  ]);
});

dbTest("list prefix with small batch size and limit", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(
    db.list({ prefix: ["a"] }, { batchSize: 2, limit: 3 }),
  );
  assertEquals(entries, [
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
  ]);
});

dbTest("list prefix with small batch size and limit reverse", async (db) => {
  const versionstamp = await setupData(db);
  const entries = await collect(
    db.list({ prefix: ["a"] }, { batchSize: 2, limit: 3, reverse: true }),
  );
  assertEquals(entries, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
  ]);
});

dbTest("list prefix with manual cursor", async (db) => {
  const versionstamp = await setupData(db);
  const iterator = db.list({ prefix: ["a"] }, { limit: 2 });
  const values = await collect(iterator);
  assertEquals(values, [
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
  ]);

  const cursor = iterator.cursor;
  assertEquals(cursor, "AmIA");

  const iterator2 = db.list({ prefix: ["a"] }, { cursor });
  const values2 = await collect(iterator2);
  assertEquals(values2, [
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "e"], value: 4, versionstamp },
  ]);
});

dbTest("list prefix with manual cursor reverse", async (db) => {
  const versionstamp = await setupData(db);

  const iterator = db.list({ prefix: ["a"] }, { limit: 2, reverse: true });
  const values = await collect(iterator);
  assertEquals(values, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
  ]);

  const cursor = iterator.cursor;
  assertEquals(cursor, "AmQA");

  const iterator2 = db.list({ prefix: ["a"] }, { cursor, reverse: true });
  const values2 = await collect(iterator2);
  assertEquals(values2, [
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "a"], value: 0, versionstamp },
  ]);
});

dbTest("list range", async (db) => {
  const versionstamp = await setupData(db);

  const entries = await collect(
    db.list({ start: ["a", "a"], end: ["a", "z"] }),
  );
  assertEquals(entries, [
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "e"], value: 4, versionstamp },
  ]);
});

dbTest("list range reverse", async (db) => {
  const versionstamp = await setupData(db);

  const entries = await collect(
    db.list({ start: ["a", "a"], end: ["a", "z"] }, { reverse: true }),
  );
  assertEquals(entries, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "a"], value: 0, versionstamp },
  ]);
});

dbTest("list range with limit", async (db) => {
  const versionstamp = await setupData(db);

  const entries = await collect(
    db.list({ start: ["a", "a"], end: ["a", "z"] }, { limit: 3 }),
  );
  assertEquals(entries, [
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
  ]);
});

dbTest("list range with limit reverse", async (db) => {
  const versionstamp = await setupData(db);

  const entries = await collect(
    db.list({ start: ["a", "a"], end: ["a", "z"] }, {
      limit: 3,
      reverse: true,
    }),
  );
  assertEquals(entries, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
  ]);
});

dbTest("list range nesting", async (db) => {
  const versionstamp = await setupData(db);

  const entries = await collect(db.list({ start: ["a"], end: ["a", "d"] }));
  assertEquals(entries, [
    { key: ["a"], value: -1, versionstamp },
    { key: ["a", "a"], value: 0, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
  ]);
});

dbTest("list range short", async (db) => {
  const versionstamp = await setupData(db);

  const entries = await collect(
    db.list({ start: ["a", "b"], end: ["a", "d"] }),
  );
  assertEquals(entries, [
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
  ]);
});

dbTest("list range with manual cursor", async (db) => {
  const versionstamp = await setupData(db);

  const iterator = db.list({ start: ["a", "b"], end: ["a", "z"] }, {
    limit: 2,
  });
  const entries = await collect(iterator);
  assertEquals(entries, [
    { key: ["a", "b"], value: 1, versionstamp },
    { key: ["a", "c"], value: 2, versionstamp },
  ]);

  const cursor = iterator.cursor;
  const iterator2 = db.list({ start: ["a", "b"], end: ["a", "z"] }, {
    cursor,
  });
  const entries2 = await collect(iterator2);
  assertEquals(entries2, [
    { key: ["a", "d"], value: 3, versionstamp },
    { key: ["a", "e"], value: 4, versionstamp },
  ]);
});

dbTest("list range with manual cursor reverse", async (db) => {
  const versionstamp = await setupData(db);

  const iterator = db.list({ start: ["a", "b"], end: ["a", "z"] }, {
    limit: 2,
    reverse: true,
  });
  const entries = await collect(iterator);
  assertEquals(entries, [
    { key: ["a", "e"], value: 4, versionstamp },
    { key: ["a", "d"], value: 3, versionstamp },
  ]);

  const cursor = iterator.cursor;
  const iterator2 = db.list({ start: ["a", "b"], end: ["a", "z"] }, {
    cursor,
    reverse: true,
  });
  const entries2 = await collect(iterator2);
  assertEquals(entries2, [
    { key: ["a", "c"], value: 2, versionstamp },
    { key: ["a", "b"], value: 1, versionstamp },
  ]);
});

dbTest("list range with start greater than end", async (db) => {
  await setupData(db);
  await assertRejects(
    async () => await collect(db.list({ start: ["b"], end: ["a"] })),
    TypeError,
    "Start key is greater than end key",
  );
});

dbTest("list range with start equal to end", async (db) => {
  await setupData(db);
  const entries = await collect(db.list({ start: ["a"], end: ["a"] }));
  assertEquals(entries.length, 0);
});

dbTest("list invalid selector", async (db) => {
  await setupData(db);

  await assertRejects(async () => {
    await collect(
      db.list({ prefix: ["a"], start: ["a", "b"], end: ["a", "c"] }),
    );
  }, TypeError);

  await assertRejects(async () => {
    await collect(
      // @ts-expect-error missing end
      db.list({ start: ["a", "b"] }),
    );
  }, TypeError);

  await assertRejects(async () => {
    await collect(
      // @ts-expect-error missing start
      db.list({ end: ["a", "b"] }),
    );
  }, TypeError);
});

dbTest("invalid versionstamp in atomic check rejects", async (db) => {
  await assertRejects(async () => {
    await db.atomic().check({ key: ["a"], versionstamp: "" }).commit();
  }, TypeError);

  await assertRejects(async () => {
    await db.atomic().check({ key: ["a"], versionstamp: "xx".repeat(10) })
      .commit();
  }, TypeError);

  await assertRejects(async () => {
    await db.atomic().check({ key: ["a"], versionstamp: "aa".repeat(11) })
      .commit();
  }, TypeError);
});

dbTest("invalid mutation type rejects", async (db) => {
  await assertRejects(async () => {
    await db.atomic()
      // @ts-expect-error invalid type + value combo
      .mutate({ key: ["a"], type: "set" })
      .commit();
  }, TypeError);

  await assertRejects(async () => {
    await db.atomic()
      // @ts-expect-error invalid type + value combo
      .mutate({ key: ["a"], type: "delete", value: "123" })
      .commit();
  }, TypeError);

  await assertRejects(async () => {
    await db.atomic()
      // @ts-expect-error invalid type
      .mutate({ key: ["a"], type: "foobar" })
      .commit();
  }, TypeError);

  await assertRejects(async () => {
    await db.atomic()
      // @ts-expect-error invalid type
      .mutate({ key: ["a"], type: "foobar", value: "123" })
      .commit();
  }, TypeError);
});

dbTest("key ordering", async (db) => {
  await db.atomic()
    .set([new Uint8Array(0x1)], 0)
    .set(["a"], 0)
    .set([1n], 0)
    .set([3.14], 0)
    .set([false], 0)
    .set([true], 0)
    .commit();

  assertEquals((await collect(db.list({ prefix: [] }))).map((x) => x.key), [
    [new Uint8Array(0x1)],
    ["a"],
    [1n],
    [3.14],
    [false],
    [true],
  ]);
});

dbTest("key size limit", async (db) => {
  // 1 byte prefix + 1 byte suffix + 2045 bytes key
  const lastValidKey = new Uint8Array(2046).fill(1);
  const firstInvalidKey = new Uint8Array(2047).fill(1);

  const res = await db.set([lastValidKey], 1);

  assertEquals(await db.get([lastValidKey]), {
    key: [lastValidKey],
    value: 1,
    versionstamp: res.versionstamp,
  });

  await assertRejects(
    async () => await db.set([firstInvalidKey], 1),
    TypeError,
    "Key too large for write (max 2048 bytes)",
  );

  await assertRejects(
    async () => await db.get([firstInvalidKey]),
    TypeError,
    "Key too large for read (max 2049 bytes)",
  );
});

dbTest("value size limit", async (db) => {
  const lastValidValue = new Uint8Array(65536);
  const firstInvalidValue = new Uint8Array(65537);

  const res = await db.set(["a"], lastValidValue);
  assertEquals(await db.get(["a"]), {
    key: ["a"],
    value: lastValidValue,
    versionstamp: res.versionstamp,
  });

  await assertRejects(
    async () => await db.set(["b"], firstInvalidValue),
    TypeError,
    "Value too large (max 65536 bytes)",
  );
});

dbTest("operation size limit", async (db) => {
  const lastValidKeys: Deno.KvKey[] = new Array(10).fill(0).map((
    _,
    i,
  ) => ["a", i]);
  const firstInvalidKeys: Deno.KvKey[] = new Array(11).fill(0).map((
    _,
    i,
  ) => ["a", i]);
  const invalidCheckKeys: Deno.KvKey[] = new Array(101).fill(0).map((
    _,
    i,
  ) => ["a", i]);

  const res = await db.getMany(lastValidKeys);
  assertEquals(res.length, 10);

  await assertRejects(
    async () => await db.getMany(firstInvalidKeys),
    TypeError,
    "Too many ranges (max 10)",
  );

  const res2 = await collect(db.list({ prefix: ["a"] }, { batchSize: 1000 }));
  assertEquals(res2.length, 0);

  await assertRejects(
    async () => await collect(db.list({ prefix: ["a"] }, { batchSize: 1001 })),
    TypeError,
    "Too many entries (max 1000)",
  );

  // when batchSize is not specified, limit is used but is clamped to 500
  assertEquals(
    (await collect(db.list({ prefix: ["a"] }, { limit: 1001 }))).length,
    0,
  );

  const res3 = await db.atomic()
    .check(...lastValidKeys.map((key) => ({
      key,
      versionstamp: null,
    })))
    .mutate(...lastValidKeys.map((key) => ({
      key,
      type: "set",
      value: 1,
    } satisfies Deno.KvMutation)))
    .commit();
  assert(res3);

  await assertRejects(
    async () => {
      await db.atomic()
        .check(...invalidCheckKeys.map((key) => ({
          key,
          versionstamp: null,
        })))
        .mutate(...lastValidKeys.map((key) => ({
          key,
          type: "set",
          value: 1,
        } satisfies Deno.KvMutation)))
        .commit();
    },
    TypeError,
    "Too many checks (max 100)",
  );

  const validMutateKeys: Deno.KvKey[] = new Array(1000).fill(0).map((
    _,
    i,
  ) => ["a", i]);
  const invalidMutateKeys: Deno.KvKey[] = new Array(1001).fill(0).map((
    _,
    i,
  ) => ["a", i]);

  const res4 = await db.atomic()
    .check(...lastValidKeys.map((key) => ({
      key,
      versionstamp: null,
    })))
    .mutate(...validMutateKeys.map((key) => ({
      key,
      type: "set",
      value: 1,
    } satisfies Deno.KvMutation)))
    .commit();
  assert(res4);

  await assertRejects(
    async () => {
      await db.atomic()
        .check(...lastValidKeys.map((key) => ({
          key,
          versionstamp: null,
        })))
        .mutate(...invalidMutateKeys.map((key) => ({
          key,
          type: "set",
          value: 1,
        } satisfies Deno.KvMutation)))
        .commit();
    },
    TypeError,
    "Too many mutations (max 1000)",
  );
});

dbTest("total mutation size limit", async (db) => {
  const keys: Deno.KvKey[] = new Array(1000).fill(0).map((
    _,
    i,
  ) => ["a", i]);

  const atomic = db.atomic();
  for (const key of keys) {
    atomic.set(key, "foo");
  }
  const res = await atomic.commit();
  assert(res);

  // Use bigger values to trigger "total mutation size too large" error
  await assertRejects(
    async () => {
      const value = new Array(3000).fill("a").join("");
      const atomic = db.atomic();
      for (const key of keys) {
        atomic.set(key, value);
      }
      await atomic.commit();
    },
    TypeError,
    "Total mutation size too large (max 819200 bytes)",
  );
});

dbTest("total key size limit", async (db) => {
  const longString = new Array(1100).fill("a").join("");
  const keys: Deno.KvKey[] = new Array(80).fill(0).map(() => [longString]);

  const atomic = db.atomic();
  for (const key of keys) {
    atomic.set(key, "foo");
  }
  await assertRejects(
    () => atomic.commit(),
    TypeError,
    "Total key size too large (max 81920 bytes)",
  );
});

dbTest("keys must be arrays", async (db) => {
  await assertRejects(
    // @ts-expect-error invalid type
    async () => await db.get("a"),
    TypeError,
  );

  await assertRejects(
    // @ts-expect-error invalid type
    async () => await db.getMany(["a"]),
    TypeError,
  );

  await assertRejects(
    // @ts-expect-error invalid type
    async () => await db.set("a", 1),
    TypeError,
  );

  await assertRejects(
    // @ts-expect-error invalid type
    async () => await db.delete("a"),
    TypeError,
  );

  await assertRejects(
    async () =>
      await db.atomic()
        // @ts-expect-error invalid type
        .mutate({ key: "a", type: "set", value: 1 } satisfies Deno.KvMutation)
        .commit(),
    TypeError,
  );

  await assertRejects(
    async () =>
      await db.atomic()
        // @ts-expect-error invalid type
        .check({ key: "a", versionstamp: null })
        .set(["a"], 1)
        .commit(),
    TypeError,
  );
});

Deno.test("Deno.Kv constructor throws", () => {
  assertThrows(() => {
    new Deno.Kv();
  });
});

// This function is never called, it is just used to check that all the types
// are behaving as expected.
async function _typeCheckingTests() {
  const kv = new Deno.Kv();

  const a = await kv.get(["a"]);
  assertType<IsExact<typeof a, Deno.KvEntryMaybe<unknown>>>(true);

  const b = await kv.get<string>(["b"]);
  assertType<IsExact<typeof b, Deno.KvEntryMaybe<string>>>(true);

  const c = await kv.getMany([["a"], ["b"]]);
  assertType<
    IsExact<typeof c, [Deno.KvEntryMaybe<unknown>, Deno.KvEntryMaybe<unknown>]>
  >(true);

  const d = await kv.getMany([["a"], ["b"]] as const);
  assertType<
    IsExact<typeof d, [Deno.KvEntryMaybe<unknown>, Deno.KvEntryMaybe<unknown>]>
  >(true);

  const e = await kv.getMany<[string, number]>([["a"], ["b"]]);
  assertType<
    IsExact<typeof e, [Deno.KvEntryMaybe<string>, Deno.KvEntryMaybe<number>]>
  >(true);

  const keys: Deno.KvKey[] = [["a"], ["b"]];
  const f = await kv.getMany(keys);
  assertType<IsExact<typeof f, Deno.KvEntryMaybe<unknown>[]>>(true);

  const g = kv.list({ prefix: ["a"] });
  assertType<IsExact<typeof g, Deno.KvListIterator<unknown>>>(true);
  const h = await g.next();
  assert(!h.done);
  assertType<IsExact<typeof h.value, Deno.KvEntry<unknown>>>(true);

  const i = kv.list<string>({ prefix: ["a"] });
  assertType<IsExact<typeof i, Deno.KvListIterator<string>>>(true);
  const j = await i.next();
  assert(!j.done);
  assertType<IsExact<typeof j.value, Deno.KvEntry<string>>>(true);
}

queueTest("basic listenQueue and enqueue", async (db) => {
  const { promise, resolve } = Promise.withResolvers<void>();
  let dequeuedMessage: unknown = null;
  const listener = db.listenQueue((msg) => {
    dequeuedMessage = msg;
    resolve();
  });
  try {
    const res = await db.enqueue("test");
    assert(res.ok);
    assertNotEquals(res.versionstamp, null);
    await promise;
    assertEquals(dequeuedMessage, "test");
  } finally {
    db.close();
    await listener;
  }
});

for (const { name, value } of VALUE_CASES) {
  queueTest(`listenQueue and enqueue ${name}`, async (db) => {
    const numEnqueues = 10;
    let count = 0;
    const deferreds: ReturnType<typeof Promise.withResolvers<unknown>>[] = [];
    const listeners: Promise<void>[] = [];
    listeners.push(db.listenQueue((msg: unknown) => {
      deferreds[count++].resolve(msg);
    }));
    try {
      for (let i = 0; i < numEnqueues; i++) {
        deferreds.push(Promise.withResolvers<unknown>());
        await db.enqueue(value);
      }
      const dequeuedMessages = await Promise.all(
        deferreds.map(({ promise }) => promise),
      );
      for (let i = 0; i < numEnqueues; i++) {
        assertEquals(dequeuedMessages[i], value);
      }
    } finally {
      db.close();
      for (const listener of listeners) {
        await listener;
      }
    }
  });
}

queueTest("queue mixed types", async (db) => {
  let deferred: ReturnType<typeof Promise.withResolvers<void>>;
  let dequeuedMessage: unknown = null;
  const listener = db.listenQueue((msg: unknown) => {
    dequeuedMessage = msg;
    deferred.resolve();
  });
  try {
    for (const item of VALUE_CASES) {
      deferred = Promise.withResolvers<void>();
      await db.enqueue(item.value);
      await deferred.promise;
      assertEquals(dequeuedMessage, item.value);
    }
  } finally {
    db.close();
    await listener;
  }
});

queueTest("queue delay", async (db) => {
  let dequeueTime: number | undefined;
  const { promise, resolve } = Promise.withResolvers<void>();
  let dequeuedMessage: unknown = null;
  const listener = db.listenQueue((msg) => {
    dequeueTime = Date.now();
    dequeuedMessage = msg;
    resolve();
  });
  try {
    const enqueueTime = Date.now();
    await db.enqueue("test", { delay: 1000 });
    await promise;
    assertEquals(dequeuedMessage, "test");
    assert(dequeueTime !== undefined);
    assert(dequeueTime - enqueueTime >= 1000);
  } finally {
    db.close();
    await listener;
  }
});

queueTest("queue delay with atomic", async (db) => {
  let dequeueTime: number | undefined;
  const { promise, resolve } = Promise.withResolvers<void>();
  let dequeuedMessage: unknown = null;
  const listener = db.listenQueue((msg) => {
    dequeueTime = Date.now();
    dequeuedMessage = msg;
    resolve();
  });
  try {
    const enqueueTime = Date.now();
    const res = await db.atomic()
      .enqueue("test", { delay: 1000 })
      .commit();
    assert(res.ok);

    await promise;
    assertEquals(dequeuedMessage, "test");
    assert(dequeueTime !== undefined);
    assert(dequeueTime - enqueueTime >= 1000);
  } finally {
    db.close();
    await listener;
  }
});

queueTest("queue delay and now", async (db) => {
  let count = 0;
  let dequeueTime: number | undefined;
  const { promise, resolve } = Promise.withResolvers<void>();
  let dequeuedMessage: unknown = null;
  const listener = db.listenQueue((msg) => {
    count += 1;
    if (count == 2) {
      dequeueTime = Date.now();
      dequeuedMessage = msg;
      resolve();
    }
  });
  try {
    const enqueueTime = Date.now();
    await db.enqueue("test-1000", { delay: 1000 });
    await db.enqueue("test");
    await promise;
    assertEquals(dequeuedMessage, "test-1000");
    assert(dequeueTime !== undefined);
    assert(dequeueTime - enqueueTime >= 1000);
  } finally {
    db.close();
    await listener;
  }
});

dbTest("queue negative delay", async (db) => {
  await assertRejects(async () => {
    await db.enqueue("test", { delay: -100 });
  }, TypeError);
});

dbTest("queue nan delay", async (db) => {
  await assertRejects(async () => {
    await db.enqueue("test", { delay: Number.NaN });
  }, TypeError);
});

dbTest("queue large delay", async (db) => {
  await db.enqueue("test", { delay: 30 * 24 * 60 * 60 * 1000 });
  await assertRejects(async () => {
    await db.enqueue("test", { delay: 30 * 24 * 60 * 60 * 1000 + 1 });
  }, TypeError);
});

queueTest("listenQueue with async callback", async (db) => {
  const { promise, resolve } = Promise.withResolvers<void>();
  let dequeuedMessage: unknown = null;
  const listener = db.listenQueue(async (msg) => {
    dequeuedMessage = msg;
    await sleep(100);
    resolve();
  });
  try {
    await db.enqueue("test");
    await promise;
    assertEquals(dequeuedMessage, "test");
  } finally {
    db.close();
    await listener;
  }
});

queueTest("queue retries", async (db) => {
  let count = 0;
  const listener = db.listenQueue(async (_msg) => {
    count += 1;
    await sleep(10);
    throw new TypeError("dequeue error");
  });
  try {
    await db.enqueue("test");
    await sleep(10000);
  } finally {
    db.close();
    await listener;
  }

  // There should have been 1 attempt + 3 retries in the 10 seconds
  assertEquals(4, count);
});

queueTest("queue retries with backoffSchedule", async (db) => {
  let count = 0;
  const listener = db.listenQueue((_msg) => {
    count += 1;
    throw new TypeError("Dequeue error");
  });
  try {
    await db.enqueue("test", { backoffSchedule: [1] });
    await sleep(2000);
  } finally {
    db.close();
    await listener;
  }

  // There should have been 1 attempt + 1 retry
  assertEquals(2, count);
});

queueTest("multiple listenQueues", async (db) => {
  const numListens = 10;
  let count = 0;
  const deferreds: ReturnType<typeof Promise.withResolvers<void>>[] = [];
  const dequeuedMessages: unknown[] = [];
  const listeners: Promise<void>[] = [];
  for (let i = 0; i < numListens; i++) {
    listeners.push(db.listenQueue((msg) => {
      dequeuedMessages.push(msg);
      deferreds[count++].resolve();
    }));
  }
  try {
    for (let i = 0; i < numListens; i++) {
      deferreds.push(Promise.withResolvers<void>());
      await db.enqueue("msg_" + i);
      await deferreds[i].promise;
      const msg = dequeuedMessages[i];
      assertEquals("msg_" + i, msg);
    }
  } finally {
    db.close();
    for (let i = 0; i < numListens; i++) {
      await listeners[i];
    }
  }
});

queueTest("enqueue with atomic", async (db) => {
  const { promise, resolve } = Promise.withResolvers<void>();
  let dequeuedMessage: unknown = null;
  const listener = db.listenQueue((msg) => {
    dequeuedMessage = msg;
    resolve();
  });

  try {
    await db.set(["t"], "1");

    let currentValue = await db.get(["t"]);
    assertEquals("1", currentValue.value);

    const res = await db.atomic()
      .check(currentValue)
      .set(currentValue.key, "2")
      .enqueue("test")
      .commit();
    assert(res.ok);

    await promise;
    assertEquals("test", dequeuedMessage);

    currentValue = await db.get(["t"]);
    assertEquals("2", currentValue.value);
  } finally {
    db.close();
    await listener;
  }
});

queueTest("enqueue with atomic nonce", async (db) => {
  const { promise, resolve } = Promise.withResolvers<void>();
  let dequeuedMessage: unknown = null;

  const nonce = crypto.randomUUID();

  const listener = db.listenQueue(async (val) => {
    const message = val as { msg: string; nonce: string };
    const nonce = message.nonce;
    const nonceValue = await db.get(["nonces", nonce]);
    if (nonceValue.versionstamp === null) {
      dequeuedMessage = message.msg;
      resolve();
      return;
    }

    assertNotEquals(nonceValue.versionstamp, null);
    const res = await db.atomic()
      .check(nonceValue)
      .delete(["nonces", nonce])
      .set(["a", "b"], message.msg)
      .commit();
    if (res.ok) {
      // Simulate an error so that the message has to be redelivered
      throw new Error("injected error");
    }
  });

  try {
    const res = await db.atomic()
      .check({ key: ["nonces", nonce], versionstamp: null })
      .set(["nonces", nonce], true)
      .enqueue({ msg: "test", nonce })
      .commit();
    assert(res.ok);

    await promise;
    assertEquals("test", dequeuedMessage);

    const currentValue = await db.get(["a", "b"]);
    assertEquals("test", currentValue.value);

    const nonceValue = await db.get(["nonces", nonce]);
    assertEquals(nonceValue.versionstamp, null);
  } finally {
    db.close();
    await listener;
  }
});

Deno.test({
  name: "queue persistence with inflight messages",
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const filename = await Deno.makeTempFile({ prefix: "queue_db" });
    try {
      let db: Deno.Kv = await Deno.openKv(filename);

      let count = 0;
      let deferred = Promise.withResolvers<void>();

      // Register long-running handler.
      let listener = db.listenQueue(async (_msg) => {
        count += 1;
        if (count == 3) {
          deferred.resolve();
        }
        await new Promise(() => {});
      });

      // Enqueue 3 messages.
      await db.enqueue("msg0");
      await db.enqueue("msg1");
      await db.enqueue("msg2");
      await deferred.promise;

      // Close the database and wait for the listener to finish.
      db.close();
      await listener;

      // Wait at least MESSAGE_DEADLINE_TIMEOUT before reopening the database.
      // This ensures that inflight messages are requeued immediately after
      // the database is reopened.
      // https://github.com/denoland/denokv/blob/efb98a1357d37291a225ed5cf1fc4ecc7c737fab/sqlite/backend.rs#L120
      await sleep(6000);

      // Now reopen the database.
      db = await Deno.openKv(filename);

      count = 0;
      deferred = Promise.withResolvers<void>();

      // Register a handler that will complete quickly.
      listener = db.listenQueue((_msg) => {
        count += 1;
        if (count == 3) {
          deferred.resolve();
        }
      });

      // Wait for the handlers to finish.
      await deferred.promise;
      assertEquals(3, count);
      db.close();
      await listener;
    } finally {
      try {
        await Deno.remove(filename);
      } catch {
        // pass
      }
    }
  },
});

Deno.test({
  name: "queue persistence with delay messages",
  async fn() {
    const filename = await Deno.makeTempFile({ prefix: "queue_db" });
    try {
      await Deno.remove(filename);
    } catch {
      // pass
    }
    try {
      let db: Deno.Kv = await Deno.openKv(filename);

      let count = 0;
      let deferred = Promise.withResolvers<void>();

      // Register long-running handler.
      let listener = db.listenQueue((_msg) => {});

      // Enqueue 3 messages into the future.
      await db.enqueue("msg0", { delay: 10000 });
      await db.enqueue("msg1", { delay: 10000 });
      await db.enqueue("msg2", { delay: 10000 });

      // Close the database and wait for the listener to finish.
      db.close();
      await listener;

      // Now reopen the database.
      db = await Deno.openKv(filename);

      count = 0;
      deferred = Promise.withResolvers<void>();

      // Register a handler that will complete quickly.
      listener = db.listenQueue((_msg) => {
        count += 1;
        if (count == 3) {
          deferred.resolve();
        }
      });

      // Wait for the handlers to finish.
      await deferred.promise;
      assertEquals(3, count);
      db.close();
      await listener;
    } finally {
      try {
        await Deno.remove(filename);
      } catch {
        // pass
      }
    }
  },
});

Deno.test({
  name: "different kv instances for enqueue and queueListen",
  async fn() {
    const filename = await Deno.makeTempFile({ prefix: "queue_db" });
    try {
      const db0 = await Deno.openKv(filename);
      const db1 = await Deno.openKv(filename);
      const { promise, resolve } = Promise.withResolvers<void>();
      let dequeuedMessage: unknown = null;
      const listener = db0.listenQueue((msg) => {
        dequeuedMessage = msg;
        resolve();
      });
      try {
        const res = await db1.enqueue("test");
        assert(res.ok);
        assertNotEquals(res.versionstamp, null);
        await promise;
        assertEquals(dequeuedMessage, "test");
      } finally {
        db0.close();
        await listener;
        db1.close();
      }
    } finally {
      try {
        await Deno.remove(filename);
      } catch {
        // pass
      }
    }
  },
});

Deno.test({
  name: "queue graceful close",
  async fn() {
    const db: Deno.Kv = await Deno.openKv(":memory:");
    const listener = db.listenQueue((_msg) => {});
    db.close();
    await listener;
  },
});

dbTest("Invalid backoffSchedule", async (db) => {
  await assertRejects(
    async () => {
      await db.enqueue("foo", { backoffSchedule: [1, 1, 1, 1, 1, 1] });
    },
    TypeError,
    "Invalid backoffSchedule",
  );
  await assertRejects(
    async () => {
      await db.enqueue("foo", { backoffSchedule: [3600001] });
    },
    TypeError,
    "Invalid backoffSchedule",
  );
});

dbTest("atomic operation is exposed", (db) => {
  assert(Deno.AtomicOperation);
  const ao = db.atomic();
  assert(ao instanceof Deno.AtomicOperation);
});

Deno.test({
  name: "racy open",
  async fn() {
    for (let i = 0; i < 100; i++) {
      const filename = await Deno.makeTempFile({ prefix: "racy_open_db" });
      try {
        const [db1, db2, db3] = await Promise.all([
          Deno.openKv(filename),
          Deno.openKv(filename),
          Deno.openKv(filename),
        ]);
        db1.close();
        db2.close();
        db3.close();
      } finally {
        await Deno.remove(filename);
      }
    }
  },
});

Deno.test({
  name: "racy write",
  async fn() {
    const filename = await Deno.makeTempFile({ prefix: "racy_write_db" });
    const concurrency = 20;
    const iterations = 5;
    try {
      const dbs = await Promise.all(
        Array(concurrency).fill(0).map(() => Deno.openKv(filename)),
      );
      try {
        for (let i = 0; i < iterations; i++) {
          await Promise.all(
            dbs.map((db) => db.atomic().sum(["counter"], 1n).commit()),
          );
        }
        assertEquals(
          ((await dbs[0].get(["counter"])).value as Deno.KvU64).value,
          BigInt(concurrency * iterations),
        );
      } finally {
        dbs.forEach((db) => db.close());
      }
    } finally {
      await Deno.remove(filename);
    }
  },
});

Deno.test({
  name: "kv expiration",
  async fn() {
    const filename = await Deno.makeTempFile({ prefix: "kv_expiration_db" });
    try {
      await Deno.remove(filename);
    } catch {
      // pass
    }
    let db: Deno.Kv | null = null;

    try {
      db = await Deno.openKv(filename);

      await db.set(["a"], 1, { expireIn: 1000 });
      await db.set(["b"], 2, { expireIn: 1000 });
      assertEquals((await db.get(["a"])).value, 1);
      assertEquals((await db.get(["b"])).value, 2);

      // Value overwrite should also reset expiration
      await db.set(["b"], 2, { expireIn: 3600 * 1000 });

      // Wait for expiration
      await sleep(1000);

      // Re-open to trigger immediate cleanup
      db.close();
      db = null;
      db = await Deno.openKv(filename);

      let ok = false;
      for (let i = 0; i < 50; i++) {
        await sleep(100);
        if (
          JSON.stringify(
            (await db.getMany([["a"], ["b"]])).map((x) => x.value),
          ) === "[null,2]"
        ) {
          ok = true;
          break;
        }
      }

      if (!ok) {
        throw new Error("Values did not expire");
      }
    } finally {
      if (db) {
        try {
          db.close();
        } catch {
          // pass
        }
      }
      try {
        await Deno.remove(filename);
      } catch {
        // pass
      }
    }
  },
});

Deno.test({
  name: "kv expiration with atomic",
  async fn() {
    const filename = await Deno.makeTempFile({ prefix: "kv_expiration_db" });
    try {
      await Deno.remove(filename);
    } catch {
      // pass
    }
    let db: Deno.Kv | null = null;

    try {
      db = await Deno.openKv(filename);

      await db.atomic().set(["a"], 1, { expireIn: 1000 }).set(["b"], 2, {
        expireIn: 1000,
      }).commit();
      assertEquals((await db.getMany([["a"], ["b"]])).map((x) => x.value), [
        1,
        2,
      ]);

      // Wait for expiration
      await sleep(1000);

      // Re-open to trigger immediate cleanup
      db.close();
      db = null;
      db = await Deno.openKv(filename);

      let ok = false;
      for (let i = 0; i < 50; i++) {
        await sleep(100);
        if (
          JSON.stringify(
            (await db.getMany([["a"], ["b"]])).map((x) => x.value),
          ) === "[null,null]"
        ) {
          ok = true;
          break;
        }
      }

      if (!ok) {
        throw new Error("Values did not expire");
      }
    } finally {
      if (db) {
        try {
          db.close();
        } catch {
          // pass
        }
      }
      try {
        await Deno.remove(filename);
      } catch {
        // pass
      }
    }
  },
});

Deno.test({
  name: "remote backend",
  async fn() {
    const db = await Deno.openKv("http://localhost:4545/kv_remote_authorize");
    try {
      await db.set(["some-key"], 1);
      const entry = await db.get(["some-key"]);
      assertEquals(entry.value, null);
      assertEquals(entry.versionstamp, null);
    } finally {
      db.close();
    }
  },
});

Deno.test({
  name: "remote backend invalid format",
  async fn() {
    const db = await Deno.openKv(
      "http://localhost:4545/kv_remote_authorize_invalid_format",
    );

    await assertRejects(
      async () => {
        await db.set(["some-key"], 1);
      },
      Error,
      "Failed to parse metadata: ",
    );

    db.close();
  },
});

Deno.test({
  name: "remote backend invalid version",
  async fn() {
    const db = await Deno.openKv(
      "http://localhost:4545/kv_remote_authorize_invalid_version",
    );

    await assertRejects(
      async () => {
        await db.set(["some-key"], 1);
      },
      Error,
      "Failed to parse metadata: unsupported metadata version: 1000",
    );

    db.close();
  },
});

Deno.test(
  { permissions: { read: true } },
  async function kvExplicitResourceManagement() {
    let kv2: Deno.Kv;

    {
      using kv = await Deno.openKv(":memory:");
      kv2 = kv;

      const res = await kv.get(["a"]);
      assertEquals(res.versionstamp, null);
    }

    await assertRejects(() => kv2.get(["a"]), Deno.errors.BadResource);
  },
);

Deno.test(
  { permissions: { read: true } },
  async function kvExplicitResourceManagementManualClose() {
    using kv = await Deno.openKv(":memory:");
    kv.close();
    await assertRejects(() => kv.get(["a"]), Deno.errors.BadResource);
    // calling [Symbol.dispose] after manual close is a no-op
  },
);

dbTest("key watch", async (db) => {
  const changeHistory: Deno.KvEntryMaybe<number>[] = [];
  const watcher: ReadableStream<Deno.KvEntryMaybe<number>[]> = db.watch<
    number[]
  >([["key"]]);

  const reader = watcher.getReader();
  const expectedChanges = 2;

  const work = (async () => {
    for (let i = 0; i < expectedChanges; i++) {
      const message = await reader.read();
      if (message.done) {
        throw new Error("Unexpected end of stream");
      }
      changeHistory.push(message.value[0]);
    }

    await reader.cancel();
  })();

  while (changeHistory.length !== 1) {
    await sleep(100);
  }
  assertEquals(changeHistory[0], {
    key: ["key"],
    value: null,
    versionstamp: null,
  });

  const { versionstamp } = await db.set(["key"], 1);
  while (changeHistory.length as number !== 2) {
    await sleep(100);
  }
  assertEquals(changeHistory[1], {
    key: ["key"],
    value: 1,
    versionstamp,
  });

  await work;
  await reader.cancel();
});

dbTest("set with key versionstamp suffix", async (db) => {
  const result1 = await Array.fromAsync(db.list({ prefix: ["a"] }));
  assertEquals(result1, []);

  const setRes1 = await db.set(["a", db.commitVersionstamp()], "b");
  assert(setRes1.ok);
  assert(setRes1.versionstamp > ZERO_VERSIONSTAMP);

  const result2 = await Array.fromAsync(db.list({ prefix: ["a"] }));
  assertEquals(result2.length, 1);
  assertEquals(result2[0].key[1], setRes1.versionstamp);
  assertEquals(result2[0].value, "b");
  assertEquals(result2[0].versionstamp, setRes1.versionstamp);

  const setRes2 = await db.atomic().set(["a", db.commitVersionstamp()], "c")
    .commit();
  assert(setRes2.ok);
  assert(setRes2.versionstamp > setRes1.versionstamp);

  const result3 = await Array.fromAsync(db.list({ prefix: ["a"] }));
  assertEquals(result3.length, 2);
  assertEquals(result3[1].key[1], setRes2.versionstamp);
  assertEquals(result3[1].value, "c");
  assertEquals(result3[1].versionstamp, setRes2.versionstamp);

  await assertRejects(
    async () => await db.set(["a", db.commitVersionstamp(), "a"], "x"),
    TypeError,
    "expected string, number, bigint, ArrayBufferView, boolean",
  );
});

Deno.test({
  name: "watch should stop when db closed",
  async fn() {
    const db = await Deno.openKv(":memory:");

    const watch = db.watch([["a"]]);
    const completion = (async () => {
      for await (const _item of watch) {
        // pass
      }
    })();

    setTimeout(() => {
      db.close();
    }, 100);

    await completion;
  },
});
