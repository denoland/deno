// { "preopens": { "/fixture": "fixture" } }

fn main() {
  let metadata = std::fs::metadata("/fixture/directory").unwrap();
  assert!(metadata.is_dir());
  assert!(metadata.len() > 0);

  let metadata = std::fs::metadata("/fixture/symlink_to_directory").unwrap();
  assert!(metadata.is_dir());
  assert!(metadata.len() > 0);

  let metadata = std::fs::metadata("/fixture/file").unwrap();
  assert!(metadata.is_file());
  assert!(metadata.len() > 0);

  let metadata = std::fs::metadata("/fixture/symlink_to_file").unwrap();
  assert!(metadata.is_file());
  assert!(metadata.len() > 0);

  let metadata = std::fs::metadata("/fixture/directory/file").unwrap();
  assert!(metadata.is_file());
  assert!(metadata.len() > 0);

  let metadata =
    std::fs::metadata("/fixture/directory/symlink_to_file").unwrap();
  assert!(metadata.is_file());
  assert!(metadata.len() > 0);
}
