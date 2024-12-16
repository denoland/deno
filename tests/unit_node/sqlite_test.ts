// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { DatabaseSync } from "node:sqlite";
import { assertEquals, assertThrows } from "@std/assert";

Deno.test("[node/sqlite] in-memory databases", () => {
  const db1 = new DatabaseSync(":memory:");
  const db2 = new DatabaseSync(":memory:");
  db1.exec("CREATE TABLE data(key INTEGER PRIMARY KEY);");
  db1.exec("INSERT INTO data (key) VALUES (1);");

  db2.exec("CREATE TABLE data(key INTEGER PRIMARY KEY);");
  db2.exec("INSERT INTO data (key) VALUES (1);");

  assertEquals(db1.prepare("SELECT * FROM data").all(), [{
    key: 1,
    __proto__: null,
  }]);
  assertEquals(db2.prepare("SELECT * FROM data").all(), [{
    key: 1,
    __proto__: null,
  }]);
});

Deno.test("[node/sqlite] Errors originating from SQLite should be thrown", () => {
  const db = new DatabaseSync(":memory:");
  db.exec(`
    CREATE TABLE test(
      key INTEGER PRIMARY KEY
    ) STRICT;
  `);
  const stmt = db.prepare("INSERT INTO test(key) VALUES(?)");
  assertEquals(stmt.run(1), { lastInsertRowid: 1, changes: 1 });

  assertThrows(() => stmt.run(1), Error);
});

Deno.test(
  {
    permissions: { read: true, write: true },
    name: "[node/sqlite] PRAGMAs are supported",
  },
  () => {
    const tempDir = Deno.makeTempDirSync();
    const db = new DatabaseSync(`${tempDir}/test.db`);

    assertEquals(db.prepare("PRAGMA journal_mode = WAL").get(), {
      journal_mode: "wal",
      __proto__: null,
    });

    db.close();
    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test("[node/sqlite] StatementSync read bigints are supported", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data(key INTEGER PRIMARY KEY);");
  db.exec("INSERT INTO data (key) VALUES (1);");

  const stmt = db.prepare("SELECT * FROM data");
  assertEquals(stmt.get(), { key: 1, __proto__: null });

  stmt.setReadBigInts(true);
  assertEquals(stmt.get(), { key: 1n, __proto__: null });
});
