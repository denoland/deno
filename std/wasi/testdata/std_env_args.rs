// { "args": ["one", "two", "three" ]}

fn main() {
  let mut args = std::env::args();
  assert_eq!(args.len(), 3);
  assert_eq!(args.next().unwrap(), "one");
  assert_eq!(args.next().unwrap(), "two");
  assert_eq!(args.next().unwrap(), "three");
}
