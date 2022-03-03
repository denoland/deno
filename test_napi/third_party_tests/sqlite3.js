const { Database } = require("sqlite3").verbose();
const db = new Database(":memory:");

db.close();
