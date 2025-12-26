import { DatabaseSync } from "node:sqlite";

using db = new DatabaseSync(":memory:");

// Test 1: Basic numbered parameters (?1, ?2)
console.log("Test 1: Basic numbered parameters (?1, ?2)");
db.exec(`
  CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL
  )
`);

const insertStmt = db.prepare("INSERT INTO users (name, email) VALUES (?1, ?2)");
const result = insertStmt.run("Alice", "alice@example.com");
console.log("Insert changes:", result.changes);

const row = db.prepare("SELECT name, email FROM users WHERE id = 1").get() as { name: string; email: string };
console.log("Inserted row:", row.name, row.email);

// Test 2: Parameter reuse (?1 appearing multiple times)
console.log("\nTest 2: Parameter reuse (?1 appearing multiple times)");
db.exec(`
  CREATE TABLE nodes (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER
  )
`);
db.exec("INSERT INTO nodes (id, parent_id) VALUES (1, NULL), (2, 1), (3, 1), (4, 2)");

const reuseStmt = db.prepare("SELECT * FROM nodes WHERE id = ?1 OR parent_id = ?1 ORDER BY id");
const rows = reuseStmt.all(1) as { id: number; parent_id: number | null }[];
console.log("Rows found:", rows.length);
for (const r of rows) {
  console.log(`  id=${r.id}, parent_id=${r.parent_id}`);
}

// Test 3: Parameters in different order (?2, ?1)
console.log("\nTest 3: Parameters in different order (?2, ?1)");
db.exec("CREATE TABLE test (a TEXT, b TEXT)");

const orderStmt = db.prepare("INSERT INTO test (a, b) VALUES (?2, ?1)");
orderStmt.run("first_arg", "second_arg");

const testRow = db.prepare("SELECT a, b FROM test").get() as { a: string; b: string };
// first_arg binds to ?1, second_arg binds to ?2
// SQL puts ?2 in column a, ?1 in column b
console.log("Column a:", testRow.a);
console.log("Column b:", testRow.b);

console.log("\nAll tests passed!");
