export function test() {
  console.log("1");
  console.log("2");
  console.log("3");
  console.log("4");
  console.log("5");
  console.log("6");
  console.log("7");
  console.log("8");
  console.log("9");
  return 42;
}
if (import.meta.main) {
  test();
}
