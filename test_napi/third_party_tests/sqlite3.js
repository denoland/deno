const sqlite3 = Deno.core.napiOpen(
  "node_modules/sqlite3/build-tmp-napi-v3/Release/node_sqlite3.node",
);

function normalizeMethod(fn) {
  return function (sql) {
    let errBack;
    const args = Array.prototype.slice.call(arguments, 1);
    if (typeof args[args.length - 1] === "function") {
      const callback = args[args.length - 1];
      errBack = function (err) {
        if (err) {
          callback(err);
        }
      };
    }
    const statement = new sqlite3.Statement(this, sql, errBack);
    return fn.call(this, statement, args);
  };
}

let Database = sqlite3.Database;
Database.prototype.run = normalizeMethod(function (statement, params) {
  return params.length ? statement.run.apply(statement, params) : statement;
});

const db = new Database(":memory:");

db.serialize(function () {
  console.log(db);
  db.run("CREATE TABLE lorem (info TEXT)");
});

db.close();
