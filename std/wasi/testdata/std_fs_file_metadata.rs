// { "preopens": { "/fixture": "fixture" } }

fn main() {
  let file = std::fs::File::open("/fixture/file").unwrap();
  let metadata = file.metadata().unwrap();
  assert!(metadata.is_file());
  assert!(metadata.len() == 5);

  let file = std::fs::File::open("/fixture/symlink_to_file").unwrap();
  let metadata = file.metadata().unwrap();
  assert!(metadata.is_file());
  assert!(metadata.len() == 5);

  let file = std::fs::File::open("/fixture/directory/file").unwrap();
  let metadata = file.metadata().unwrap();
  assert!(metadata.is_file());
  assert!(metadata.len() == 15);

  let file = std::fs::File::open("/fixture/directory/symlink_to_file").unwrap();
  let metadata = file.metadata().unwrap();
  assert!(metadata.is_file());
  assert!(metadata.len() == 15);
}
