import { DatabaseSync } from "node:sqlite";

const database = new DatabaseSync(":memory:");

// Execute SQL statements from strings.
database.exec(`
    CREATE TABLE data(
      key INTEGER PRIMARY KEY,
      value TEXT
    ) STRICT
  `);

// Create a prepared statement to insert data into the database.
const insert = database.prepare(
  "INSERT INTO data (key, value) VALUES (:key, :val)",
);
// Execute the prepared statement with bound values.
insert.run({ key: 2, val: "world" });

insert.run({ val: "hello", key: 1 }); // Problem occurs here

// Create a prepared statement to read data from the database.
const query = database.prepare("SELECT * FROM data ORDER BY key");
// Execute the prepared statement and log the result set.
console.log(query.all());
