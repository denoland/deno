// { "args": ["one", "two", "three" ]}

fn main() {
  let args = std::env::args();
  assert_eq!(args.len(), 3);
}
