import { DatabaseSync } from "node:sqlite";

const db = new DatabaseSync(":memory:");
db.exec("ATTACH DATABASE ':memory:' AS test");
db.exec("CREATE TABLE test.foo (id INTEGER PRIMARY KEY)");
db.close();
console.log("OK");
