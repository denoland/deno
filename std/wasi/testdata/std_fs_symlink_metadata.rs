// { "preopens": { "/fixture": "fixture" } }

fn main() {
  let metadata = std::fs::symlink_metadata("/fixture/directory").unwrap();
  assert!(metadata.file_type().is_dir());

  let metadata =
    std::fs::symlink_metadata("/fixture/symlink_to_directory").unwrap();
  assert!(metadata.file_type().is_symlink());

  let metadata = std::fs::symlink_metadata("/fixture/file").unwrap();
  assert!(metadata.file_type().is_file());

  let metadata = std::fs::symlink_metadata("/fixture/symlink_to_file").unwrap();
  assert!(metadata.file_type().is_symlink());

  let metadata = std::fs::symlink_metadata("/fixture/directory/file").unwrap();
  assert!(metadata.file_type().is_file());

  let metadata =
    std::fs::symlink_metadata("/fixture/directory/symlink_to_file").unwrap();
  assert!(metadata.file_type().is_symlink());
}
