// { "preopens": { "/fixture": "fixture", "/scratch": "scratch" } }

fn main() {
  assert!(
    std::fs::hard_link("/fixture/file", "/scratch/hardlink_to_file").is_ok()
  );
  assert_eq!(
    std::fs::read("/fixture/file").unwrap(),
    std::fs::read("/scratch/hardlink_to_file").unwrap()
  );
}
