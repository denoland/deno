// Runs with --allow-all. With full permissions for the database path the
// attach cap is not in force, so both the `limits.attach` constructor option
// and the `db.limits.attach` setter raise the limit and ATTACH DATABASE then
// succeeds. This is the regression guard for the scoped-permission fix: it
// fails if the cap is ever applied unconditionally.
import { DatabaseSync } from "node:sqlite";

const allowedDb = "allowed/main.db";
const deniedDb = "denied/secret.db";

function readsSecret(db) {
  db.exec(`ATTACH DATABASE '${deniedDb}' AS other`);
  return db.prepare("SELECT v FROM other.secrets").get().v;
}

// The `limits.attach` constructor option raises the limit and ATTACH works.
{
  const db = new DatabaseSync(allowedDb, { limits: { attach: 1 } });
  console.log("ctor-attach-limit:" + db.limits.attach);
  console.log("ctor-attach:" + readsSecret(db));
  db.close();
}

// The `db.limits.attach` setter raises the limit and ATTACH works.
{
  const db = new DatabaseSync(allowedDb);
  db.limits.attach = 1;
  console.log("setter-attach-limit:" + db.limits.attach);
  console.log("setter-attach:" + readsSecret(db));
  db.close();
}
