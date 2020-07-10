// { "preopens": { "/scratch": "scratch" } }

fn main() {
  assert!(std::fs::create_dir("/scratch/directory").is_ok());
  assert!(std::fs::metadata("/scratch/directory").unwrap().is_dir());
}
