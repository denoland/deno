import { DB } from "https://deno.land/x/sqlite/mod.ts";
const db = new DB("/tmp/northwind.sqlite");

{
  const sql = db.prepareQuery(`SELECT * FROM "Order"`);
  Deno.bench('SELECT * FROM "Order"', () => {
    sql.allEntries();
  });
}

{
  const sql = db.prepareQuery(`SELECT * FROM "Product"`);
  Deno.bench('SELECT * FROM "Product"', () => {
    sql.allEntries();
  });
}

{
  const sql = db.prepareQuery(`SELECT * FROM "OrderDetail"`);
  Deno.bench('SELECT * FROM "OrderDetail"', () => {
    sql.allEntries();
  });
}

