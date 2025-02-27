import { DatabaseSync } from "node:sqlite";

const db = new DatabaseSync(":memory:");
const s = db.prepare("SELECT sqlite_version()").get();
console.log(s)
db.exec("ATTACH DATABASE 'test.db' AS test");
