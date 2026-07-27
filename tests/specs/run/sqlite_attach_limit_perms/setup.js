// Creates the fixture used by probe.js: an "allowed" directory the probe may
// touch, and a "denied" directory holding a SQLite database the probe must not
// be able to reach. Run with -A only to build the fixture.
import { DatabaseSync } from "node:sqlite";

Deno.mkdirSync("allowed", { recursive: true });
Deno.mkdirSync("denied", { recursive: true });

const db = new DatabaseSync("denied/secret.db");
db.exec("CREATE TABLE secrets(v TEXT)");
db.exec("INSERT INTO secrets VALUES ('DENIED_SECRET')");
db.close();

console.log("setup-ok");
