// { "preopens": { "/scratch": "scratch" }, "files": { "scratch/file": "file" } }

use std::io::Write;

fn main() {
  let mut file = std::fs::File::create("/scratch/file").unwrap();
  assert_eq!(file.write(b"fi").unwrap(), 2);
  assert_eq!(file.write(b"le").unwrap(), 2);
}
