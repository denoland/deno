// { "stdin": "Hello, stdin!" }

use std::io::Read;

fn main() {
  let mut buffer = String::new();
  assert!(std::io::stdin().read_to_string(&mut buffer).is_ok());
  assert_eq!(buffer, "Hello, stdin!")
}
