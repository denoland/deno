// { "stderr": "Hello, stderr!" }

use std::io::Write;

fn main() {
  assert!(std::io::stderr().write_all(b"Hello, stderr!").is_ok())
}
