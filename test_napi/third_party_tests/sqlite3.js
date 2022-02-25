const sqlite3 = Deno.core.napiOpen(
  "node_modules/sqlite3/build-tmp-napi-v3/Release/node_sqlite3.node",
);

const db = new sqlite3.Database(':memory:');

db.serialize(function() {
  console.log(db)
  (new sqlite3.Statement(this, "CREATE TABLE lorem (info TEXT)")).run([]);

});

db.close();