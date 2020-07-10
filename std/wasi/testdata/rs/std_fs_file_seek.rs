// { "preopens": { "/fixture": "fixture" } }

use std::io::Seek;

fn main() {
  let mut file = std::fs::File::open("/fixture/file").unwrap();
  assert_eq!(file.seek(std::io::SeekFrom::Current(0)).unwrap(), 0);

  assert_eq!(file.seek(std::io::SeekFrom::Start(1)).unwrap(), 1);
  assert_eq!(file.seek(std::io::SeekFrom::Start(2)).unwrap(), 2);
  assert_eq!(file.seek(std::io::SeekFrom::Start(3)).unwrap(), 3);
  assert_eq!(file.seek(std::io::SeekFrom::Start(4)).unwrap(), 4);
  assert_eq!(file.seek(std::io::SeekFrom::Start(5)).unwrap(), 5);

  assert_eq!(file.seek(std::io::SeekFrom::Current(-1)).unwrap(), 4);
  assert_eq!(file.seek(std::io::SeekFrom::Current(-1)).unwrap(), 3);
  assert_eq!(file.seek(std::io::SeekFrom::Current(-1)).unwrap(), 2);
  assert_eq!(file.seek(std::io::SeekFrom::Current(-1)).unwrap(), 1);
  assert_eq!(file.seek(std::io::SeekFrom::Current(-1)).unwrap(), 0);

  assert_eq!(file.seek(std::io::SeekFrom::Current(1)).unwrap(), 1);
  assert_eq!(file.seek(std::io::SeekFrom::Current(1)).unwrap(), 2);
  assert_eq!(file.seek(std::io::SeekFrom::Current(1)).unwrap(), 3);
  assert_eq!(file.seek(std::io::SeekFrom::Current(1)).unwrap(), 4);
  assert_eq!(file.seek(std::io::SeekFrom::Current(1)).unwrap(), 5);

  assert_eq!(file.seek(std::io::SeekFrom::End(0)).unwrap(), 5);
  assert_eq!(file.seek(std::io::SeekFrom::End(-1)).unwrap(), 4);
  assert_eq!(file.seek(std::io::SeekFrom::End(-2)).unwrap(), 3);
  assert_eq!(file.seek(std::io::SeekFrom::End(-3)).unwrap(), 2);
  assert_eq!(file.seek(std::io::SeekFrom::End(-4)).unwrap(), 1);
  assert_eq!(file.seek(std::io::SeekFrom::End(-5)).unwrap(), 0);
}
