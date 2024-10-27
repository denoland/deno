// deno-coverage-ignore-file

export function unused(condition: boolean): boolean {
  if (condition) {
    return false;
  } else {
    return true;
  }
}
