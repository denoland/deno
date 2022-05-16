import { bench, run } from "https://esm.run/mitata";

const { Connection } = Deno.sqlite;
const conn = new Connection("/Users/divy/Desktop/Northwind_large.sqlite")

bench('SELECT * From "Order"', () => {
  const stmt = conn.prepare(`SELECT * FROM "Order"`);
  stmt.query();
});

run();
