export function foo() {
  // deno-coverage-ignore-start
  if (1 > 2) {
    // deno-coverage-ignore-start
    console.log("That's odd!");
    // deno-coverage-ignore-stop
  }
  // deno-coverage-ignore-stop
}
