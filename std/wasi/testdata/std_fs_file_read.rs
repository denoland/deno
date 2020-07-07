// { "preopens": { "/fixture": "fixture" } }

use std::io::Read;

fn main() {
  let mut file = std::fs::File::open("/fixture/file").unwrap();
  let mut buffer = [0; 2];

  assert_eq!(file.read(&mut buffer).unwrap(), 2);
  assert_eq!(&buffer, b"fi");

  assert_eq!(file.read(&mut buffer).unwrap(), 2);
  assert_eq!(&buffer, b"le");

  assert_eq!(file.read(&mut buffer).unwrap(), 1);
}
