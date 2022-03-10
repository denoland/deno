use async_trait::async_trait;

#[async_trait]
trait IDB {
  async fn open_database(&self, name: String, version: Option<u64>) -> (u64, u64);
  async fn list_databases(&self) -> Vec<String>;
}
