// NOTE: Ensure that source maps are handled correctly by adding a type which
//       will be removed during transpilation
type Dummy = {
  _: string;
};

export function used(condition: boolean): boolean {
  // deno-coverage-ignore-start
  if (condition) {
    return false;
  }
  // deno-coverage-ignore-stop

  return true;
}

// deno-coverage-ignore-start
function unused() {
  console.log("unused");
}
