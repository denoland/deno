// { "preopens": { "/scratch": "scratch" } }

fn main() {
  let old_path = "/scratch/old_file";
  let new_path = "/scratch/new_file";

  assert!(std::fs::write(old_path, b"Hello, world!").is_ok());
  assert!(std::fs::rename(old_path, new_path).is_ok());
  assert!(std::fs::read(old_path).is_err());
  assert_eq!(std::fs::read(new_path).unwrap(), b"Hello, world!");
}
