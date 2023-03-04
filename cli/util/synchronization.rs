use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

pub struct SingleConcurrencyEnforcerPermit<'a>(SemaphorePermit<'a>);

/// Enforces nothing else can run the code within the
/// duration of the permit.
#[derive(Debug)]
pub struct SingleConcurrencyEnforcer(Semaphore);

impl SingleConcurrencyEnforcer {
  pub fn new() -> Self {
    Self(Semaphore::new(1))
  }

  pub async fn acquire(&self) -> SingleConcurrencyEnforcerPermit {
    let permit = self.0.acquire().await.unwrap();
    SingleConcurrencyEnforcerPermit(permit)
  }
}

impl Default for SingleConcurrencyEnforcer {
  fn default() -> Self {
    Self::new()
  }
}
