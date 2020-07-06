// { "preopens": { "/fixture": "fixture" } }

fn main() {
  let entries = std::fs::read_dir("/fixture").unwrap();
  assert_eq!(entries.count(), 4);

  let entries = std::fs::read_dir("/fixture/directory").unwrap();
  assert_eq!(entries.count(), 2);
}
