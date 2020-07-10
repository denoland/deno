// { "preopens": { "/fixture": "fixture" } }

fn main() {
  assert_eq!(std::fs::read("/fixture/file").unwrap(), b"file\n");
  assert_eq!(
    std::fs::read("/fixture/symlink_to_file").unwrap(),
    b"file\n"
  );
  assert_eq!(
    std::fs::read("/fixture/directory/file").unwrap(),
    b"directory/file\n"
  );
  assert_eq!(
    std::fs::read("/fixture/directory/symlink_to_file").unwrap(),
    b"directory/file\n"
  );
}
