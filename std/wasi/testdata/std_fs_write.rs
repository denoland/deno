// { "preopens": { "/scratch": "scratch" }, "files": { "scratch/file": "file" } }

fn main() {
  assert!(std::fs::write("/scratch/file", b"file").is_ok())
}
