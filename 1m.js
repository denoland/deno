const { Connection } = Deno.sqlite;

const conn = new Connection("inserts.db");
{
  // Pragmas
  const smth = conn.prepare(
  `PRAGMA journal_mode = OFF;
PRAGMA synchronous = 0;
PRAGMA cache_size = 1000000;
PRAGMA locking_mode = EXCLUSIVE;
PRAGMA temp_store = MEMORY;`
  );
  smth.query();

  const table = conn.prepare(
  `CREATE TABLE IF NOT EXISTS user (
  id INTEGER not null primary key,
  age INTEGER not null,
  active INTEGER not null
);`
  );
  table.run();
}

const insertion = conn.prepare(`INSERT INTO user VALUES (?, ?, ?);`);
for (let i = 0; i < 1_000_000; i++) {
  insertion.run(i, 17, 1);
}
