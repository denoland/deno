use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::ZeroCopyBuf;

#[async_trait]
pub trait IDB {
  async fn open_database(&self, name: String, version: Option<u64>) -> Result<(u64, u64), AnyError>;
  async fn list_databases(&self) -> Result<Vec<String>, AnyError>;

  async fn object_store_rename(&self, database: String, store: String, new_name: String) -> Result<(), AnyError>;
  async fn object_store_put(&self, database: String, store: String, value: ZeroCopyBuf, key: Option<ZeroCopyBuf>) -> Result<(), AnyError>;
  async fn object_store_add(&self, database: String, store: String, value: ZeroCopyBuf, key: Option<ZeroCopyBuf>) -> Result<(), AnyError>;
  async fn object_store_delete(&self, database: String, store: String, query: ) -> Result<(), AnyError>;
  async fn object_store_clear(&self, database: String, store: String) -> Result<(), AnyError>;
  async fn object_store_get(&self, database: String, store: String, query: ) -> Result<(), AnyError>;
}
