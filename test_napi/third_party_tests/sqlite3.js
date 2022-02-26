const { Database } = Deno.core.napiOpen(
  "node_modules/sqlite3/build-tmp-napi-v3/Release/node_sqlite3.node",
);
const db = new Database(":memory:");
db.close();
