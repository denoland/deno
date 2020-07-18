// { "preopens": { "/scratch": "scratch" } }

fn main() {
  let file = std::fs::File::create("/scratch/file").unwrap();

  assert!(file.set_len(0).is_ok());
  assert_eq!(file.metadata().unwrap().len(), 0);

  assert!(file.set_len(5).is_ok());
  assert_eq!(file.metadata().unwrap().len(), 5);

  assert!(file.set_len(25).is_ok());
  assert_eq!(file.metadata().unwrap().len(), 25);

  assert!(file.set_len(0).is_ok());
  assert_eq!(file.metadata().unwrap().len(), 0);
}
