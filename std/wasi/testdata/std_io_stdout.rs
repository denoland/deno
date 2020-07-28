// { "stdout": "Hello, stdout!" }

use std::io::Write;

fn main() {
  assert!(std::io::stdout().write_all(b"Hello, stdout!").is_ok())
}
