export function add(a, b) {
  return a + b;
}

if (import.meta.main) {
  console.log("Add 2 + 3", add(2, 3));
}
