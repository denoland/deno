// Copyright 2018-2025 the Deno authors. MIT license.
import sqlite, { DatabaseSync } from "node:sqlite";
import { assert, assertEquals, assertThrows } from "@std/assert";

const tempDir = Deno.makeTempDirSync();

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
    const db = new DatabaseSync(`${tempDir}/test.db`);

    assertEquals(db.prepare("PRAGMA journal_mode = WAL").get(), {
      journal_mode: "wal",
      __proto__: null,
    });

    db.close();
  },
);

Deno.test("[node/sqlite] StatementSync bind bigints", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data(key INTEGER PRIMARY KEY);");

  const stmt = db.prepare("INSERT INTO data (key) VALUES (?)");
  assertEquals(stmt.run(100n), { lastInsertRowid: 100, changes: 1 });
  db.close();
});

Deno.test("[node/sqlite] StatementSync read bigints are supported", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data(key INTEGER PRIMARY KEY);");
  db.exec("INSERT INTO data (key) VALUES (1);");

  const stmt = db.prepare("SELECT * FROM data");
  assertEquals(stmt.get(), { key: 1, __proto__: null });

  stmt.setReadBigInts(true);
  assertEquals(stmt.get(), { key: 1n, __proto__: null });

  assertEquals(stmt.sourceSQL, "SELECT * FROM data");
  assertEquals(stmt.expandedSQL, "SELECT * FROM data");
});

Deno.test("[node/sqlite] createSession and changesets", () => {
  const db = new DatabaseSync(":memory:");
  const session = db.createSession();

  db.exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)");
  db.exec("INSERT INTO test (name) VALUES ('foo')");

  assert(session.changeset() instanceof Uint8Array);
  assert(session.patchset() instanceof Uint8Array);

  assert(session.changeset().byteLength > 0);
  assert(session.patchset().byteLength > 0);

  session.close();

  // Use after close shoud throw.
  assertThrows(() => session.changeset(), Error, "Session is already closed");
  // Close after close should throw.
  assertThrows(() => session.close(), Error, "Session is already closed");

  db.close();
  assertThrows(() => session.close(), Error, "Database is already closed");
});

Deno.test("[node/sqlite] StatementSync integer too large", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data(key INTEGER PRIMARY KEY);");
  db.prepare("INSERT INTO data (key) VALUES (?)").run(
    Number.MAX_SAFE_INTEGER + 1,
  );

  assertThrows(() => db.prepare("SELECT * FROM data").get());
});

Deno.test("[node/sqlite] StatementSync blob are Uint8Array", () => {
  const db = new DatabaseSync(":memory:");
  const obj = db.prepare("select cast('test' as blob)").all();

  assertEquals(obj.length, 1);
  const row = obj[0] as Record<string, Uint8Array>;
  assert(row["cast('test' as blob)"] instanceof Uint8Array);
});

Deno.test({
  name: "[node/sqlite] sqlite permissions",
  permissions: { read: false, write: false },
  fn() {
    assertThrows(() => {
      new DatabaseSync("test.db");
    }, Deno.errors.NotCapable);
    assertThrows(() => {
      new DatabaseSync("test.db", { readOnly: true });
    }, Deno.errors.NotCapable);
  },
});

Deno.test({
  name: "[node/sqlite] readOnly database",
  permissions: { read: true, write: true },
  fn() {
    {
      const db = new DatabaseSync(`${tempDir}/test3.db`);
      db.exec("CREATE TABLE foo (id INTEGER PRIMARY KEY)");
      db.close();
    }
    {
      const db = new DatabaseSync(`${tempDir}/test3.db`, { readOnly: true });
      assertThrows(
        () => {
          db.exec("CREATE TABLE test(key INTEGER PRIMARY KEY)");
        },
        Error,
        "attempt to write a readonly database",
      );
      db.close();
    }
    {
      const db = new DatabaseSync(":memory:");
      assertThrows(
        () => {
          db.exec("ATTACH DATABASE 'test.db' AS test");
        },
        Error,
        "too many attached databases - max 0",
      );
      db.close();
    }
  },
});

Deno.test("[node/sqlite] applyChangeset across databases", () => {
  const sourceDb = new DatabaseSync(":memory:");
  const targetDb = new DatabaseSync(":memory:");

  sourceDb.exec("CREATE TABLE data(key INTEGER PRIMARY KEY, value TEXT)");
  targetDb.exec("CREATE TABLE data(key INTEGER PRIMARY KEY, value TEXT)");

  const session = sourceDb.createSession();

  const insert = sourceDb.prepare(
    "INSERT INTO data (key, value) VALUES (?, ?)",
  );
  insert.run(1, "hello");
  insert.run(2, "world");

  const changeset = session.changeset();
  targetDb.applyChangeset(changeset, {
    filter: (e) => e === "data",
    // @ts-ignore: types are not up to date
    onConflict: () => sqlite.constants.SQLITE_CHANGESET_ABORT,
  });

  const stmt = targetDb.prepare("SELECT * FROM data");
  assertEquals(stmt.all(), [
    { key: 1, value: "hello", __proto__: null },
    { key: 2, value: "world", __proto__: null },
  ]);
});

Deno.test("[node/sqlite] exec should execute batch statements", () => {
  const db = new DatabaseSync(":memory:");
  db.exec(`CREATE TABLE one(id int PRIMARY KEY) STRICT;
CREATE TABLE two(id int PRIMARY KEY) STRICT;`);

  const table = db.prepare(
    `SELECT name FROM sqlite_master WHERE type='table'`,
  ).all();
  assertEquals(table.length, 2);

  db.close();
});

Deno.test("[node/sqlite] query should handle mixed positional and named parameters", () => {
  const db = new DatabaseSync(":memory:");
  db.exec(`CREATE TABLE one(variable1 TEXT, variable2 INT, variable3 INT)`);
  db.exec(
    `INSERT INTO one (variable1, variable2, variable3) VALUES ("test", 1 , 2);`,
  );

  const query = "SELECT * FROM one WHERE variable1=:var1 AND variable2=:var2 ";
  const result = db.prepare(query).all({ var1: "test", var2: 1 });
  assertEquals(result, [{
    __proto__: null,
    variable1: "test",
    variable2: 1,
    variable3: 2,
  }]);

  const result2 = db.prepare(query).all({ var2: 1, var1: "test" });
  assertEquals(result2, [{
    __proto__: null,
    variable1: "test",
    variable2: 1,
    variable3: 2,
  }]);

  const stmt = db.prepare(query);
  stmt.setAllowBareNamedParameters(false);
  assertThrows(() => {
    stmt.all({ var1: "test", var2: 1 });
  });

  db.close();
});

Deno.test("[node/sqlite] StatementSync#iterate", () => {
  const db = new DatabaseSync(":memory:");
  const stmt = db.prepare("SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3");
  // @ts-ignore: types are not up to date
  const iter = stmt.iterate();

  const result = [];
  for (const row of iter) {
    result.push(row);
  }

  assertEquals(result, stmt.all());

  const { done, value } = iter.next();
  assertEquals(done, true);
  assertEquals(value, undefined);

  db.close();
});

// https://github.com/denoland/deno/issues/28187
Deno.test("[node/sqlite] StatementSync for large integers", () => {
  const db = new DatabaseSync(":memory:");
  const result = db.prepare("SELECT 2147483648").get();
  assertEquals(result, { "2147483648": 2147483648, __proto__: null });
  db.close();
});

Deno.test("[node/sqlite] error message", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE foo (a text, b text NOT NULL, c text)");

  assertThrows(
    () => {
      db.prepare("INSERT INTO foo(a, b, c) VALUES (NULL, NULL, NULL)")
        .run();
    },
    Error,
    "NOT NULL constraint failed: foo.b",
  );
});

// https://github.com/denoland/deno/issues/28295
Deno.test("[node/sqlite] StatementSync reset guards don't lock db", () => {
  const db = new DatabaseSync(":memory:");

  db.exec("CREATE TABLE foo(a integer, b text)");
  db.exec("CREATE TABLE bar(a integer, b text)");

  const stmt = db.prepare("SELECT name FROM sqlite_master WHERE type='table' ");

  assertEquals(stmt.get(), { name: "foo", __proto__: null });

  db.exec("DROP TABLE IF EXISTS foo");
});

// https://github.com/denoland/deno/issues/28492
Deno.test("[node/sqlite] StatementSync reset step change metadata", () => {
  const db = new DatabaseSync(":memory:");

  db.exec(`CREATE TABLE people (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  birthdate TEXT NOT NULL
) STRICT`);

  const insertPeople = db.prepare(`
INSERT INTO people
  (name, birthdate)
VALUES
  (:name, :birthdate)
RETURNING id
`);

  const id1 = insertPeople.run({ name: "Flash", birthdate: "1956-07-16" });
  assertEquals(id1, { lastInsertRowid: 1, changes: 1 });
});

Deno.test("[node/sqlite] StatementSync empty blob", () => {
  const db = new DatabaseSync(":memory:");

  db.exec("CREATE TABLE foo(a BLOB NOT NULL)");

  db.prepare("INSERT into foo (a) values (?)")
    .all(new Uint8Array([]));

  const result = db.prepare("SELECT * from foo").get();
  assertEquals(result, { a: new Uint8Array([]), __proto__: null });

  db.close();
});
