use crate::deno_buf;
use crate::shared_simple::SharedSimpleRecord;

/// Represents a shared buffer that can be accessed by JS.
pub trait Shared<Record = SharedSimpleRecord> {
  fn as_deno_buf(&self) -> deno_buf;

  /// Returns static JS code which implements Deno.shared
  /// This will be executed and available to Isolates before
  /// other code is run.
  fn js() -> (&'static str, &'static str);

  /// Pushes a record onto the stack. Returns false if no room.
  fn push(&mut self, record: &Record) -> bool;

  /// Returns the last element of the stack if any.
  fn pop(&mut self) -> Option<Record>;

  /// Removes all records.
  /// Also resets the next().
  fn reset(&mut self);

  /// Returns number of elements.
  fn len(&self) -> usize;
}
