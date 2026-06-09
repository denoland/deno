// Copyright 2018-2026 the Deno authors. MIT license.
import { strictEqual } from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";

const gc = (globalThis as { gc?: () => void }).gc;
strictEqual(typeof gc, "function");

function collect() {
  for (let i = 0; i < 10; i++) {
    gc!();
    const values = [];
    for (let j = 0; j < 100_000; j++) {
      values.push(j);
    }
    strictEqual(values.length, 100_000);
  }
}

function assertStatementRefIsHidden(iterator: Iterator<unknown>) {
  const iteratorObject = iterator as Iterator<unknown> & {
    __statement_ref?: unknown;
  };
  const descriptor = Object.getOwnPropertyDescriptor(
    iteratorObject,
    "__statement_ref",
  );
  strictEqual(descriptor?.configurable, false);
  strictEqual(descriptor?.writable, false);
  strictEqual(descriptor?.enumerable, false);

  try {
    delete iteratorObject.__statement_ref;
  } catch {
    // Modules are strict, so deleting a non-configurable property may throw.
  }
  try {
    iteratorObject.__statement_ref = undefined;
  } catch {
    // Modules are strict, so assigning to a read-only property may throw.
  }

  const descriptorAfter = Object.getOwnPropertyDescriptor(
    iteratorObject,
    "__statement_ref",
  );
  strictEqual(descriptorAfter?.value, descriptor?.value);
}

function assertNextRow(
  actual: IteratorResult<{ id: number }>,
  expectedId: number,
) {
  strictEqual(actual.done, false);
  strictEqual(actual.value.id, expectedId);
  strictEqual(Object.getPrototypeOf(actual), null);
  strictEqual(Object.getPrototypeOf(actual.value), null);
}

function assertDone(actual: IteratorResult<{ id: number }>) {
  strictEqual(actual.done, true);
  strictEqual(actual.value, null);
  strictEqual(Object.getPrototypeOf(actual), null);
}

{
  let db: DatabaseSync | null = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE test (id INTEGER PRIMARY KEY)");
  db.exec("INSERT INTO test (id) VALUES (1), (2)");
  // deno-lint-ignore no-explicit-any
  let stmt: any = db.prepare("SELECT id FROM test ORDER BY id ASC");
  let iterator: Iterator<{ id: number }> | null = stmt.iterate();
  assertStatementRefIsHidden(iterator);

  const next = iterator.next;
  iterator = null;
  stmt = null;
  db = null;

  collect();
  assertNextRow(next(), 1);
  assertNextRow(next(), 2);
  assertDone(next());
}

{
  let db: DatabaseSync | null = new DatabaseSync(":memory:");
  db.exec("CREATE TABLE test (id INTEGER PRIMARY KEY)");
  db.exec("INSERT INTO test (id) VALUES (1), (2)");
  // @ts-expect-error createTagStore is a valid method
  const store = db.createTagStore(10);
  // deno-lint-ignore no-explicit-any
  let sql: any = store;
  let iterator: Iterator<{ id: number }> | null = sql
    .iterate`SELECT id FROM test ORDER BY id ASC`;
  assertStatementRefIsHidden(iterator);

  const next = iterator.next;
  iterator = null;
  sql = null;
  db = null;

  collect();
  assertNextRow(next(), 1);
  assertNextRow(next(), 2);
  assertDone(next());
}
