import { bench, run } from "https://esm.sh/mitata";

const { Connection } = Deno.sqlite;
const conn = new Connection("/Users/divy/Desktop/Northwind_large.sqlite");
{
const stmt = conn.prepare('SELECT * From "Order"');
bench('SELECT * From "Order"', () => {
  stmt.query();
});
}
{
const stmt = conn.prepare('SELECT * From "Product"');
bench('SELECT * From "Product"', () => {
  stmt.query();
});
}
{
const stmt = conn.prepare('SELECT * From "OrderDetail"');
bench('SELECT * From "OrderDetail"', () => {
  stmt.query();
});
}
run();
