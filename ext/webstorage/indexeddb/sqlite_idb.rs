use deno_core::error::AnyError;
use rusqlite::params;
use rusqlite::Connection;

struct SqliteIDB(Connection);

impl super::idbtrait::IDB for SqliteIDB {
  async fn open_database(
    &self,
    name: String,
    version: Option<u64>,
  ) -> Result<(u64, u64), AnyError> {
    todo!()
  }

  async fn list_databases(&self) -> Result<Vec<String>, AnyError> {
    let mut stmt = self.0.prepare_cached("SELECT name FROM database")?;
    let names = stmt
      .query(params![])?
      .map(|row| row.get(0).unwrap())
      .collect::<Vec<String>>()?;
    Ok(names)
  }
}
