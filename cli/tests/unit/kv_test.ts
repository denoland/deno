// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects } from "./test_util.ts";

Deno.test("kv basic", async () => {
  const db: Deno.Database = await Deno.openDatabase(
    ":memory:",
  );
  try {
    // Basic JSON values & versionstamp emulation
    await db.set(["a"], "b");
    {
      const out = await db.get(["a"]);
      assertEquals(out.value, "b");
      assertEquals(out.versionstamp, "00000000000000010000");
    }

    await db.set(["a"], "c");
    {
      const out = await db.get(["a"]);
      assertEquals(out.value, "c");
      assertEquals(out.versionstamp, "00000000000000020000");
    }

    // Basic binary values
    await db.set(["a"], new Uint8Array([1, 2, 3]));
    {
      const out = await db.get(["a"]);
      assertEquals(out.value, new Uint8Array([1, 2, 3]));
      assertEquals(out.versionstamp, "00000000000000030000");
    }

    // Object values
    await db.set(["a"], { a: 1, b: 2 });
    {
      const out = await db.get(["a"]);
      assertEquals(out.value, { a: 1, b: 2 });
    }

    // BigInt, boolean, null, undefined, and Date values
    await db.set(["a"], [BigInt(42), true, null, undefined, new Date(0)]);
    {
      const out = await db.get(["a"]);
      assertEquals(out.value, [BigInt(42), true, null, undefined, new Date(0)]);
    }

    // clear value
    await db.delete(["a"]);
    {
      const out = await db.get(["a"]);
      assertEquals(out.value, null);
      assertEquals(out.versionstamp, null);
    }

    // compare and mutate
    {
      await db.set(["t"], "1");

      const currentValue = await db.get(["t"]);
      assertEquals(currentValue.value, "1");

      let ok = await db.atomic()
        .check(currentValue)
        .set(currentValue.key, "2")
        .commit();
      assertEquals(ok, true);

      ok = await db.atomic()
        .check(currentValue)
        .set(currentValue.key, "3")
        .commit();
      assertEquals(ok, false);
    }

    // u64 value
    await db.set(["a"], new Deno.KvU64(42n));
    assertEquals((await db.get(["a"])).value, new Deno.KvU64(42n));
  } finally {
    await db.close();
  }
});

Deno.test("kv atomic mutations", async () => {
  const db: Deno.Database = await Deno.openDatabase(
    ":memory:",
  );
  try {
    await db.set(["a"], new Deno.KvU64(10n));
    assertEquals((await db.get(["a"])).value, new Deno.KvU64(10n));

    let ok = await db.atomic().mutate({
      key: ["a"],
      value: new Deno.KvU64(1n),
      type: "sum",
    }).commit();
    assertEquals(ok, true);
    assertEquals((await db.get(["a"])).value, new Deno.KvU64(11n));

    ok = await db.atomic().mutate({
      key: ["a"],
      value: new Deno.KvU64(20n),
      type: "max",
    }).commit();
    assertEquals(ok, true);
    assertEquals((await db.get(["a"])).value, new Deno.KvU64(20n));

    ok = await db.atomic().mutate({
      key: ["a"],
      value: new Deno.KvU64(19n),
      type: "max",
    }).commit();
    assertEquals(ok, true);
    assertEquals((await db.get(["a"])).value, new Deno.KvU64(20n));

    ok = await db.atomic().mutate({
      key: ["a"],
      value: new Deno.KvU64(18n),
      type: "min",
    }).commit();
    assertEquals(ok, true);
    assertEquals((await db.get(["a"])).value, new Deno.KvU64(18n));

    // Overflow wrapping
    ok = await db.atomic().mutate({
      key: ["a"],
      value: new Deno.KvU64(0xffffffffffffffffn),
      type: "sum",
    }).commit();
    assertEquals(ok, true);
    assertEquals((await db.get(["a"])).value, new Deno.KvU64(17n));

    // non-u64 values should be rejected
    await db.set(["b"], 42);
    await assertRejects(
      async () => {
        await db.atomic().mutate({
          key: ["b"],
          value: new Deno.KvU64(1n),
          type: "sum",
        }).commit();
      },
      Error,
      "Cannot perform operation 'sum' on a non-U64 value",
    );
    await assertRejects(
      async () => {
        await db.atomic().mutate({
          key: ["a"],
          value: 1,
          type: "sum",
        }).commit();
      },
      Error,
      "Cannot perform operation 'sum' with a non-U64 operand",
    );
  } finally {
    await db.close();
  }
});

Deno.test("kv list", async () => {
  const db: Deno.Database = await Deno.openDatabase(
    ":memory:",
  );
  try {
    await db.set(["a", "a"], 0);
    await db.set(["a", "b"], 1);
    await db.set(["a", "c"], 2);
    await db.set(["a", "d"], 3);
    await db.set(["a", "e"], 4);
    await db.set(["b", "a"], 100);

    // prefix listing
    {
      const out: { key: Deno.KvKey; value: unknown }[] = [];
      for await (const entry of db.list({ prefix: ["a"] })) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "a"], value: 0 },
        { key: ["a", "b"], value: 1 },
        { key: ["a", "c"], value: 2 },
        { key: ["a", "d"], value: 3 },
        { key: ["a", "e"], value: 4 },
      ]);
    }

    // reverse prefix listing
    {
      const out: { key: Deno.KvKey; value: unknown }[] = [];
      for await (const entry of db.list({ prefix: ["a"] }, { reverse: true })) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "e"], value: 4 },
        { key: ["a", "d"], value: 3 },
        { key: ["a", "c"], value: 2 },
        { key: ["a", "b"], value: 1 },
        { key: ["a", "a"], value: 0 },
      ]);
    }

    // range listing
    {
      const out: { key: Deno.KvKey; value: unknown }[] = [];
      for await (
        const entry of db.list({ start: ["a", "b"], end: ["a", "d"] })
      ) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "b"], value: 1 },
        { key: ["a", "c"], value: 2 },
      ]);
    }

    // reverse range listing
    {
      const out: { key: Deno.KvKey; value: unknown }[] = [];
      for await (
        const entry of db.list({ start: ["a", "b"], end: ["a", "d"] }, {
          reverse: true,
        })
      ) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "c"], value: 2 },
        { key: ["a", "b"], value: 1 },
      ]);
    }

    // prefix listing with multiple batches
    {
      const out: { key: Deno.KvKey; value: unknown }[] = [];
      for await (const entry of db.list({ prefix: ["a"] }, { batchSize: 2 })) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "a"], value: 0 },
        { key: ["a", "b"], value: 1 },
        { key: ["a", "c"], value: 2 },
        { key: ["a", "d"], value: 3 },
        { key: ["a", "e"], value: 4 },
      ]);
    }

    // reverse prefix listing with multiple batches
    {
      const out: { key: Deno.KvKey; value: unknown }[] = [];
      for await (
        const entry of db.list({ prefix: ["a"] }, {
          batchSize: 2,
          reverse: true,
        })
      ) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "e"], value: 4 },
        { key: ["a", "d"], value: 3 },
        { key: ["a", "c"], value: 2 },
        { key: ["a", "b"], value: 1 },
        { key: ["a", "a"], value: 0 },
      ]);
    }

    // prefix listing with multiple batches and limited size
    {
      const out: { key: Deno.KvKey; value: unknown }[] = [];
      for await (
        const entry of db.list({ prefix: ["a"] }, { limit: 4, batchSize: 2 })
      ) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "a"], value: 0 },
        { key: ["a", "b"], value: 1 },
        { key: ["a", "c"], value: 2 },
        { key: ["a", "d"], value: 3 },
      ]);
    }

    // reverse prefix listing with multiple batches and limited size
    {
      const out: { key: Deno.KvKey; value: unknown }[] = [];
      for await (
        const entry of db.list({ prefix: ["a"] }, {
          limit: 4,
          batchSize: 2,
          reverse: true,
        })
      ) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "e"], value: 4 },
        { key: ["a", "d"], value: 3 },
        { key: ["a", "c"], value: 2 },
        { key: ["a", "b"], value: 1 },
      ]);
    }

    // prefix listing with manual cursors
    {
      let out: { key: Deno.KvKey; value: unknown }[] = [];
      let it = db.list({ prefix: ["a"] }, { limit: 3 });
      for await (const entry of it) {
        out.push({ key: entry.key, value: entry.value });
      }
      assertEquals(out, [
        { key: ["a", "a"], value: 0 },
        { key: ["a", "b"], value: 1 },
        { key: ["a", "c"], value: 2 },
      ]);

      const cursor = it.cursor();
      it = db.list({ prefix: ["a"] }, { limit: 3, cursor });
      out = [];

      for await (const entry of it) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "d"], value: 3 },
        { key: ["a", "e"], value: 4 },
      ]);
    }

    // reverse prefix listing with manual cursors
    {
      let out: { key: Deno.KvKey; value: unknown }[] = [];
      let it = db.list({ prefix: ["a"] }, { limit: 3, reverse: true });
      for await (const entry of it) {
        out.push({ key: entry.key, value: entry.value });
      }
      assertEquals(out, [
        { key: ["a", "e"], value: 4 },
        { key: ["a", "d"], value: 3 },
        { key: ["a", "c"], value: 2 },
      ]);

      const cursor = it.cursor();
      it = db.list({ prefix: ["a"] }, { limit: 3, cursor, reverse: true });
      out = [];

      for await (const entry of it) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "b"], value: 1 },
        { key: ["a", "a"], value: 0 },
      ]);
    }

    // range listing with manual cursors
    {
      const sel: Deno.KvListSelector = { start: ["a", "b"], end: ["a", "z"] };
      let out: { key: Deno.KvKey; value: unknown }[] = [];
      let it = db.list(sel, { limit: 2 });
      for await (const entry of it) {
        out.push({ key: entry.key, value: entry.value });
      }
      assertEquals(out, [
        { key: ["a", "b"], value: 1 },
        { key: ["a", "c"], value: 2 },
      ]);

      const cursor = it.cursor();
      it = db.list(sel, { limit: 1, cursor });
      out = [];

      for await (const entry of it) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "d"], value: 3 },
      ]);
    }

    // reverse range listing with manual cursors
    {
      const sel: Deno.KvListSelector = { start: ["a"], end: ["a", "e"] };
      let out: { key: Deno.KvKey; value: unknown }[] = [];
      let it = db.list(sel, { limit: 2, reverse: true });
      for await (const entry of it) {
        out.push({ key: entry.key, value: entry.value });
      }
      assertEquals(out, [
        { key: ["a", "d"], value: 3 },
        { key: ["a", "c"], value: 2 },
      ]);

      const cursor = it.cursor();
      it = db.list(sel, { limit: 1, cursor, reverse: true });
      out = [];

      for await (const entry of it) {
        out.push({ key: entry.key, value: entry.value });
      }

      assertEquals(out, [
        { key: ["a", "b"], value: 1 },
      ]);
    }
  } finally {
    await db.close();
  }
});
