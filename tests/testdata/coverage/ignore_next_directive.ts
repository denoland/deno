// NOTE: Ensure that source maps are handled correctly by adding a type which
//       will be removed during transpilation
type Dummy = {
  _: string;
};

export function used(condition: boolean): boolean {
  // deno-coverage-ignore-next
  if (condition) return false;

  return true;
}

// deno-coverage-ignore-next
function unused() {}
