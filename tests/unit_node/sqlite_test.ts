// Copyright 2018-2025 the Deno authors. MIT license.
import sqlite, { backup, DatabaseSync } from "node:sqlite";
import { assert, assertEquals, assertThrows } from "@std/assert";
import * as nodeAssert from "node:assert";
import { Buffer } from "node:buffer";
import { writeFileSync } from "node:fs";

const tempDir = Deno.makeTempDirSync();

const populate = (db: DatabaseSync, rows: number) => {
  db.exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)");
  let values = "";
  for (let i = 0; i < rows; i++) {
    values += `(${i}, 'Name ${i}'),`;
  }
  values = values.slice(0, -1); // Remove trailing comma
  db.exec(`INSERT INTO test (id, name) VALUES ${values}`);
};

Deno.test("[node/sqlite] sqlite-type symbol", () => {
  const db = new DatabaseSync(":memory:");
  const sqliteTypeSymbol = Symbol.for("sqlite-type");

  // @ts-ignore `sqliteTypeSymbol` is not available in `@types:node` for version 24.3
  assertEquals(db[sqliteTypeSymbol], "node:sqlite");

  db.close();
});

Deno.test("[node/sqlite] in-memory databases", () => {
  const db1 = new DatabaseSync(":memory:");
  const db2 = new DatabaseSync(":memory:");
  assertEquals(
    db1.exec("CREATE TABLE data(key INTEGER PRIMARY KEY);"),
    undefined,
  );
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
  assertThrows(() => session.changeset(), Error, "session is not open");
  // Close after close should throw.
  assertThrows(() => session.close(), Error, "session is not open");

  db.close();
  assertThrows(() => session.close(), Error, "database is not open");
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
      const db = new DatabaseSync(`${tempDir}/test3.db`, {
        readOnly: true,
      });
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
    `INSERT INTO one (variable1, variable2, variable3) VALUES ('test', 1 , 2);`,
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

Deno.test("[node/sqlite] StatementSync unknown named parameters should throw", () => {
  const db = new DatabaseSync(":memory:");
  db.exec(
    "CREATE TABLE foo (id INTEGER PRIMARY KEY, variable1 TEXT NOT NULL, variable2 INT NOT NULL)",
  );

  const stmt = db.prepare(
    "INSERT INTO foo (variable1, variable2) VALUES (:variable1, :variable2) RETURNING id",
  );

  assertThrows(() =>
    stmt.run(
      { variable1: "bar", variable2: 1, variable3: "baz" },
    )
  );

  db.close();
});

// https://github.com/denoland/deno/issues/31196
Deno.test("[node/sqlite] StatementSync unknown named parameters can be ignored", () => {
  const db = new DatabaseSync(":memory:");
  db.exec(
    "CREATE TABLE foo (id INTEGER PRIMARY KEY, variable1 TEXT NOT NULL, variable2 INT NOT NULL)",
  );

  const stmt = db.prepare(
    "INSERT INTO foo (variable1, variable2) VALUES (:variable1, :variable2) RETURNING id",
  );
  stmt.setAllowUnknownNamedParameters(true);

  const result = stmt.run({ variable1: "bar", variable2: 1, variable3: "baz" });

  assertEquals(result, { lastInsertRowid: 1, changes: 1 });

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

  const stmt = db.prepare(
    "SELECT name FROM sqlite_master WHERE type='table' ",
  );

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

Deno.test("[node/sqlite] Database close locks", () => {
  const db = new DatabaseSync(`${tempDir}/test4.db`);
  const statement = db.prepare(
    "CREATE TABLE test (key INTEGER PRIMARY KEY, value TEXT)",
  );
  statement.run();
  db.close();
  Deno.removeSync(`${tempDir}/test4.db`);
});

Deno.test("[node/sqlite] Database backup", async () => {
  const db = new DatabaseSync(`${tempDir}/original.db`);

  // Create a table and insert some test data
  db.exec(`
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT UNIQUE
            )
        `);
  const insertStmt = db.prepare(
    "INSERT INTO users (name, email) VALUES (?, ?)",
  );
  insertStmt.run("John Doe", "john@example.com");
  insertStmt.run("Jane Smith", "jane@example.com");
  insertStmt.run("Bob Wilson", "bob@example.com");

  // Create backup database
  await backup(db, `${tempDir}/backup.db`);

  // Verify backup contains same data
  const backupDb = new DatabaseSync(`${tempDir}/backup.db`);
  const backupSelectStmt = backupDb.prepare(
    "SELECT * FROM users ORDER BY id",
  );
  const backupUsers = backupSelectStmt.all();
  assertEquals(backupUsers.length, 3);
  assertEquals(backupUsers[0].name, "John Doe");
  assertEquals(backupUsers[0].email, "john@example.com");
  assertEquals(backupUsers[1].name, "Jane Smith");
  assertEquals(backupUsers[2].name, "Bob Wilson");

  db.close();
  backupDb.close();

  Deno.removeSync(`${tempDir}/original.db`);
  Deno.removeSync(`${tempDir}/backup.db`);
});

Deno.test("[node/sqlite] calling StatementSync methods after connection has closed", () => {
  const errMessage = "statement has been finalized";

  const db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE test (value INTEGER)");
  const stmt = db.prepare("INSERT INTO test (value) VALUES (?), (?)");
  stmt.run(1, 2);
  db.close();

  assertThrows(() => stmt.all(), Error, errMessage);
  assertThrows(() => stmt.expandedSQL, Error, errMessage);
  assertThrows(() => stmt.get(), Error, errMessage);
  assertThrows(() => stmt.iterate(), Error, errMessage);
  assertThrows(() => stmt.setAllowBareNamedParameters(true), Error, errMessage);
  assertThrows(
    () => stmt.setAllowUnknownNamedParameters(true),
    Error,
    errMessage,
  );
  assertThrows(() => stmt.setReadBigInts(true), Error, errMessage);
  assertThrows(() => stmt.sourceSQL, Error, errMessage);
});

// Regression test for https://github.com/denoland/deno/issues/30144
Deno.test("[node/sqlite] StatementSync iterate should not reuse previous state", () => {
  using db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE numbers (value INTEGER)");

  const stmt = db.prepare("SELECT value FROM numbers");
  assertEquals(Array.from(stmt.iterate()), []);

  db.exec("INSERT INTO numbers (value) VALUES (10), (20), (30)");
  assertEquals(Array.from(stmt.iterate()), [
    { value: 10, __proto__: null },
    { value: 20, __proto__: null },
    { value: 30, __proto__: null },
  ]);

  db.exec("DELETE FROM numbers");
  assertEquals(Array.from(stmt.iterate()), []);
});

Deno.test("[node/sqlite] detailed SQLite errors", () => {
  using db = new DatabaseSync(":memory:");

  nodeAssert.throws(() => db.prepare("SELECT * FROM noop"), {
    message: "no such table: noop",
    code: "ERR_SQLITE_ERROR",
    errcode: 1,
    errstr: "SQL logic error",
  });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate input validation", () => {
  using db = new DatabaseSync(":memory:");

  nodeAssert.throws(() => {
    // @ts-expect-error testing invalid input
    db.aggregate("sum", {
      result: (total) => total,
    });
  }, {
    code: "ERR_INVALID_ARG_TYPE",
    message:
      'The "options.start" argument must be a function or a primitive value.',
  });

  nodeAssert.throws(() => {
    db.aggregate("sum", {
      start: () => 0,
      // @ts-expect-error testing invalid input
      step: "not a function",
      result: (total) => total,
    });
  }, {
    code: "ERR_INVALID_ARG_TYPE",
    message: 'The "options.step" argument must be a function.',
  });

  nodeAssert.throws(() => {
    db.aggregate("sum", {
      start: 0,
      step: () => null,
      // @ts-expect-error testing invalid input
      useBigIntArguments: "",
    });
  }, {
    code: "ERR_INVALID_ARG_TYPE",
    message: /The "options\.useBigIntArguments" argument must be a boolean/,
  });

  nodeAssert.throws(() => {
    db.aggregate("sum", {
      start: 0,
      step: () => null,
      // @ts-expect-error testing invalid input
      varargs: "",
    });
  }, {
    code: "ERR_INVALID_ARG_TYPE",
    message: /The "options\.varargs" argument must be a boolean/,
  });

  nodeAssert.throws(() => {
    db.aggregate("sum", {
      start: 0,
      step: () => null,
      // @ts-expect-error testing invalid input
      directOnly: "",
    });
  }, {
    code: "ERR_INVALID_ARG_TYPE",
    message: /The "options\.directOnly" argument must be a boolean/,
  });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate varargs: supports variable number of arguments when true", () => {
  using db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data (value INTEGER)");
  db.exec("INSERT INTO data VALUES (1), (2), (3)");
  db.aggregate("sum_int", {
    start: 0,
    step: (_acc, _value, var1, var2, var3) => {
      // @ts-expect-error we know var1, var2, var3 are numbers
      return var1 + var2 + var3;
    },
    varargs: true,
  });

  const result = db.prepare("SELECT sum_int(value, 1, 2, 3) as total FROM data")
    .get();

  nodeAssert.deepStrictEqual(result, { __proto__: null, total: 6 });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate varargs: uses the max between step.length and inverse.length when false", () => {
  using db = new DatabaseSync(":memory:");
  db.exec(`
    CREATE TABLE t3(x, y);
    INSERT INTO t3 VALUES ('a', 1),
                          ('b', 2),
                          ('c', 3);
  `);

  db.aggregate("sumint", {
    start: 0,
    step: (acc, var1) => {
      // @ts-expect-error we know var1 and acc are numbers
      return var1 + acc;
    },
    inverse: (acc, var1, var2) => {
      // @ts-expect-error we know var1, var2 and acc are numbers
      return acc - var1 - var2;
    },
    varargs: false,
  });

  const result = db.prepare(`
    SELECT x, sumint(y, 10) OVER (
      ORDER BY x ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING
    ) AS sum_y
    FROM t3 ORDER BY x;
  `).all();

  nodeAssert.deepStrictEqual(result, [
    { __proto__: null, x: "a", sum_y: 3 },
    { __proto__: null, x: "b", sum_y: 6 },
    { __proto__: null, x: "c", sum_y: -5 },
  ]);

  nodeAssert.throws(() => {
    db.prepare(`
      SELECT x, sumint(y) OVER (
        ORDER BY x ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING
      ) AS sum_y
      FROM t3 ORDER BY x;
    `);
  }, {
    code: "ERR_SQLITE_ERROR",
    message: "wrong number of arguments to function sumint()",
  });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate varargs: throws if an incorrect number of arguments is provided when false", () => {
  using db = new DatabaseSync(":memory:");
  db.aggregate("sum_int", {
    start: 0,
    step: (_acc, var1, var2, var3) => {
      // @ts-expect-error we know var1 and var2 are numbers
      return var1 + var2 + var3;
    },
    varargs: false,
  });

  nodeAssert.throws(() => {
    db.prepare("SELECT sum_int(1, 2, 3, 4)").get();
  }, {
    code: "ERR_SQLITE_ERROR",
    message: "wrong number of arguments to function sum_int()",
  });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate: directOnly is false by default", () => {
  using db = new DatabaseSync(":memory:");
  db.aggregate("func", {
    start: 0,
    // @ts-expect-error we know acc and value are numbers
    step: (acc, value) => acc + value,
    // @ts-expect-error we know acc and value are numbers
    inverse: (acc, value) => acc - value,
  });
  db.exec(`
    CREATE TABLE t3(x, y);
    INSERT INTO t3 VALUES ('a', 4),
                          ('b', 5),
                          ('c', 3);
  `);

  db.exec(`
    CREATE TRIGGER test_trigger
    AFTER INSERT ON t3
    BEGIN
        SELECT func(1) OVER ();
    END;
  `);

  // TRIGGER will work fine with the window function
  db.exec("INSERT INTO t3 VALUES('d', 6)");
});

Deno.test("[node/sqlite] DatabaseSync.aggregate: SQLITE_DIRECT_ONLY flag when true", () => {
  using db = new DatabaseSync(":memory:");
  db.aggregate("func", {
    start: 0,
    // @ts-expect-error we know acc and value are numbers
    step: (acc, value) => acc + value,
    // @ts-expect-error we know acc and value are numbers
    inverse: (acc, value) => acc - value,
    directOnly: true,
  });
  db.exec(`
    CREATE TABLE t3(x, y);
    INSERT INTO t3 VALUES ('a', 4),
                          ('b', 5),
                          ('c', 3);
  `);

  db.exec(`
    CREATE TRIGGER test_trigger
    AFTER INSERT ON t3
    BEGIN
        SELECT func(1) OVER ();
    END;
  `);

  nodeAssert.throws(() => {
    db.exec("INSERT INTO t3 VALUES('d', 6)");
  }, {
    code: "ERR_SQLITE_ERROR",
    message: /unsafe use of func\(\)/,
  });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate start option as a value", () => {
  using db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data (value INTEGER)");
  db.exec("INSERT INTO data VALUES (1), (2), (3)");
  db.aggregate("sum_int", {
    start: 0,
    // @ts-expect-error we know acc and value are numbers
    step: (acc, value) => acc + value,
  });

  const result = db.prepare("SELECT sum_int(value) as total FROM data").get();

  nodeAssert.deepStrictEqual(result, { __proto__: null, total: 6 });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate start option as a function", () => {
  using db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data (value INTEGER)");
  db.exec("INSERT INTO data VALUES (1), (2), (3)");
  db.aggregate("sum_int", {
    start: () => 0,
    // @ts-expect-error we know acc and value are numbers
    step: (acc, value) => acc + value,
  });

  const result = db.prepare("SELECT sum_int(value) as total FROM data").get();

  nodeAssert.deepStrictEqual(result, { __proto__: null, total: 6 });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate start: can hold any js value", () => {
  using db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data (value INTEGER)");
  db.exec("INSERT INTO data VALUES (1), (2), (3)");
  // @ts-expect-error outdated type definition
  db.aggregate("sum_int", {
    start: () => [],
    step: (acc, value) => {
      // @ts-expect-error we know acc is an array
      return [...acc, value];
    },
    // @ts-expect-error we know acc is an array
    result: (acc) => acc.join(", "),
  });

  const result = db.prepare("SELECT sum_int(value) as total FROM data").get();

  nodeAssert.deepStrictEqual(result, { __proto__: null, total: "1, 2, 3" });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate start: if start throws an error", () => {
  using db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data (value INTEGER)");
  db.exec("INSERT INTO data VALUES (1), (2), (3)");
  db.aggregate("agg", {
    start: () => {
      throw new Error("start error");
    },
    step: () => null,
  });

  nodeAssert.throws(() => {
    db.prepare("SELECT agg()").get();
  }, {
    message: "start error",
  });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate throws if step throws an error", () => {
  using db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data (value INTEGER)");
  db.exec("INSERT INTO data VALUES (1), (2), (3)");
  db.aggregate("agg", {
    start: 0,
    step: () => {
      throw new Error("step error");
    },
  });

  nodeAssert.throws(() => {
    db.prepare("SELECT agg()").get();
  }, {
    message: "step error",
  });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate throws if result throws an error", () => {
  using db = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE data (value INTEGER)");
  db.exec("INSERT INTO data VALUES (1), (2), (3)");
  db.aggregate("sum_int", {
    start: 0,
    step: (acc, value) => {
      // @ts-expect-error we know acc and value are numbers
      return acc + value;
    },
    result: () => {
      throw new Error("result error");
    },
  });
  nodeAssert.throws(() => {
    db.prepare("SELECT sum_int(value) as result FROM data").get();
  }, {
    message: "result error",
  });
});

Deno.test("[node/sqlite] DatabaseSync.aggregate: throws an error when trying to use as window function but didn't provide options.inverse", () => {
  using db = new DatabaseSync(":memory:");
  db.exec(`
    CREATE TABLE t3(x, y);
    INSERT INTO t3 VALUES ('a', 4),
                          ('b', 5),
                          ('c', 3);
  `);

  db.aggregate("sumint", {
    start: 0,
    // @ts-expect-error we know total and nextValue are numbers
    step: (total, nextValue) => total + nextValue,
  });

  nodeAssert.throws(() => {
    db.prepare(`
      SELECT x, sumint(y) OVER (
        ORDER BY x ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING
      ) AS sum_y
      FROM t3 ORDER BY x;
    `);
  }, {
    code: "ERR_SQLITE_ERROR",
    message: "sumint() may not be used as a window function",
  });
});

Deno.test("[node/sqlite] accept Buffer paths", () => {
  const dbPath = `${tempDir}/buffer_path.db`;
  const backupPath = `${tempDir}/buffer_path_backup.db`;
  const dbPathBuffer = Buffer.from(dbPath);
  const backupPathBuffer = Buffer.from(backupPath);
  const db = new DatabaseSync(dbPathBuffer);

  db.exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)");
  db.exec("INSERT INTO test (name) VALUES ('Deno')");

  backup(db, backupPathBuffer);
  db.close();

  Deno.removeSync(dbPath);
  Deno.removeSync(backupPath);
});

Deno.test("[node/sqlite] accept URL paths", () => {
  const dbPath = `file://${tempDir}/url_path.db`;
  const backupPath = `file://${tempDir}/url_path_backup.db`;
  const dbPathUrl = new URL(dbPath);
  const backupPathUrl = new URL(backupPath);
  const db = new DatabaseSync(dbPathUrl);

  db.exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)");
  db.exec("INSERT INTO test (name) VALUES ('Deno')");

  backup(db, backupPathUrl);
  db.close();

  Deno.removeSync(dbPathUrl);
  Deno.removeSync(backupPathUrl);
});

Deno.test("[node/sqlite] database backup fails when dest file is not writable", async () => {
  const readonlyDestDb = `${tempDir}/readonly_backup.db`;
  using db = new DatabaseSync(":memory:");
  writeFileSync(readonlyDestDb, "", { mode: 0o444 });

  await nodeAssert.rejects(async () => {
    await backup(db, readonlyDestDb);
  }, {
    code: "ERR_SQLITE_ERROR",
    message: "attempt to write a readonly database",
  });

  Deno.removeSync(readonlyDestDb);
});

Deno.test("[node/sqlite] progress function to have been called at least once", async () => {
  const destDb = `${tempDir}/backup_progress.db`;
  using db = new DatabaseSync(":memory:");
  populate(db, 100);

  let totalPages: number | undefined;
  let remainingPages: number | undefined;
  await backup(db, destDb, {
    rate: 1,
    progress: (progress) => {
      totalPages ??= progress.totalPages;
      remainingPages ??= progress.remainingPages;
    },
  });

  assertEquals(typeof totalPages, "number");
  assertEquals(typeof remainingPages, "number");

  Deno.removeSync(destDb);
});

Deno.test("[node/sqlite] backup fails when progress function throws", async () => {
  const destDb = `${tempDir}/backup_progress.db`;
  using db = new DatabaseSync(":memory:");
  populate(db, 100);

  await nodeAssert.rejects(async () => {
    await backup(db, destDb, {
      rate: 1,
      progress: () => {
        throw new Error("progress error");
      },
    });
  }, {
    message: "progress error",
  });

  Deno.removeSync(destDb);
});

Deno.test("[node/sqlite] backup fails when source db is invalid", async () => {
  using database = new DatabaseSync(":memory:");
  const destDb = `${tempDir}/other_backup_progress.db`;

  await nodeAssert.rejects(async () => {
    await backup(database, destDb, {
      rate: 1,
      source: "invalid",
    });
  }, {
    message: "unknown database invalid",
  });

  Deno.removeSync(destDb);
});

Deno.test("[node/sqlite] backup fails when path cannot be opened", async () => {
  using db = new DatabaseSync(":memory:");

  await nodeAssert.rejects(async () => {
    await backup(db, `${tempDir}/invalid/backup.db`);
  }, {
    message: "unable to open database file",
  });
});

// https://github.com/nodejs/node/blob/591ba692bfe30408e6a67397e7d18bfa1b9c3561/test/parallel/test-sqlite-backup.mjs#L311-L314
Deno.test("[node/sqlite] backup has correct name and length", () => {
  assertEquals(backup.name, "backup");
  assertEquals(backup.length, 2);
});
