// Runs with scoped permissions: read/write to "allowed", explicit deny on
// "denied". With ATTACH DATABASE disabled for non --allow-all processes, none
// of the following should be able to reach the database in "denied" - including
// the `limits.attach` constructor option and the `db.limits.attach` setter,
// which previously re-enabled the disabled attach cap.
import { DatabaseSync } from "node:sqlite";

const allowedDb = "allowed/main.db";
const deniedDb = "denied/secret.db";

// Direct filesystem read is blocked by --deny-read.
try {
  Deno.readTextFileSync(deniedDb);
  console.log("direct-read-UNEXPECTED-OK");
} catch (e) {
  console.log(
    e instanceof Deno.errors.NotCapable
      ? "direct-read-blocked"
      : "direct-read-OTHER:" + e.name,
  );
}

// Opening the denied database directly is blocked by the permission check.
try {
  new DatabaseSync(deniedDb);
  console.log("direct-open-UNEXPECTED-OK");
} catch (e) {
  console.log(
    e instanceof Deno.errors.NotCapable
      ? "direct-open-blocked"
      : "direct-open-OTHER:" + e.name,
  );
}

// Default ATTACH DATABASE is blocked by the attach cap.
{
  const db = new DatabaseSync(allowedDb);
  try {
    db.exec(`ATTACH DATABASE '${deniedDb}' AS denied`);
    console.log("default-attach-UNEXPECTED-OK");
  } catch {
    console.log("default-attach-blocked");
  } finally {
    db.close();
  }
}

// The `limits.attach` constructor option must not raise the attach cap: the
// constructor rejects raising it with ERR_ACCESS_DENIED.
try {
  new DatabaseSync(allowedDb, { limits: { attach: 1 } });
  console.log("limits-attach-UNEXPECTED-OK");
} catch (e) {
  console.log(
    e?.code === "ERR_ACCESS_DENIED"
      ? "limits-attach-blocked"
      : "limits-attach-OTHER:" + e.name,
  );
}

// The `db.limits.attach` setter must not raise the attach cap: the setter
// rejects raising it with ERR_ACCESS_DENIED.
{
  const db = new DatabaseSync(allowedDb);
  try {
    db.limits.attach = 1;
    console.log("setter-attach-UNEXPECTED-OK");
  } catch (e) {
    console.log(
      e?.code === "ERR_ACCESS_DENIED"
        ? "setter-attach-blocked"
        : "setter-attach-OTHER:" + e.name,
    );
  } finally {
    db.close();
  }
}
