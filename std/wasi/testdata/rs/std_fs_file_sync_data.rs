// { "preopens": { "/scratch": "scratch" } }

use std::io::Write;

fn main() {
  let mut file = std::fs::File::create("/scratch/file").unwrap();

  assert!(file.write(b"Hello").is_ok());
  assert!(file.sync_data().is_ok());
  assert_eq!(std::fs::read("/scratch/file").unwrap(), b"Hello");

  assert!(file.write(b", world!").is_ok());
  assert!(file.sync_data().is_ok());
  assert_eq!(std::fs::read("/scratch/file").unwrap(), b"Hello, world!");
}
