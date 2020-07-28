// { "env": { "one": "1", "two": "2", "three": "3" } }

fn main() {
  let vars = std::env::vars();
  assert_eq!(vars.count(), 3);
}
