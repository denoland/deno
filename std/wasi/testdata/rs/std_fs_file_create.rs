// { "preopens": { "/scratch": "scratch" } }

fn main() {
  assert!(std::fs::File::create("/scratch/file").is_ok());
  assert!(std::fs::metadata("/scratch/file").unwrap().is_file());
}
