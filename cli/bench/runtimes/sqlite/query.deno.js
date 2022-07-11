import { DB } from "https://deno.land/x/sqlite/mod.ts";
import { bench, run } from "https://esm.run/mitata";
const db = new DB("/tmp/northwind.sqlite");

{
  const sql = db.prepareQuery(`SELECT * FROM "Order"`);
  bench('SELECT * FROM "Order"', () => {
    sql.allEntries();
  });
}

{
  const sql = db.prepareQuery(`SELECT * FROM "Product"`);
  bench('SELECT * FROM "Product"', () => {
    sql.allEntries();
  });
}

{
  const sql = db.prepareQuery(`SELECT * FROM "OrderDetail"`);
  bench('SELECT * FROM "OrderDetail"', () => {
    sql.allEntries();
  });
}

run({ json: false });
