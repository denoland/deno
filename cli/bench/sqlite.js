// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

import { DatabaseSync } from "node:sqlite";
import fs from "node:fs";

function bench(name, fun, count = 10000) {
  const start = Date.now();
  for (let i = 0; i < count; i++) fun();
  const elapsed = Date.now() - start;
  const rate = Math.floor(count / (elapsed / 1000));
  console.log(`  ${name}: time ${elapsed} ms rate ${rate}`);
}

for (const name of [":memory:", "test.db"]) {
  console.log(`Benchmarking ${name}`);
  try {
    fs.unlinkSync(name);
  } catch {
    // Ignore
  }

  const db = new DatabaseSync(name);
  db.exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)");

  bench("prepare", () => db.prepare("SELECT * FROM test"));
  bench("exec", () => db.exec("INSERT INTO test (name) VALUES ('foo')"));

  const stmt = db.prepare("SELECT * FROM test");
  bench("get", () => stmt.get());

  const stmt2 = db.prepare("SELECT * FROM test WHERE id = ?");
  bench("get (integer bind)", () => stmt2.get(1));

  bench("all", () => stmt.all(), 1000);
}
