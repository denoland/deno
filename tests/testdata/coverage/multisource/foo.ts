export function foo(cond: boolean) {
  let a = 0;
  if (cond) {
    a = 1;
  } else {
    a = 2;
  }

  if (a == 4) {
    return 1;
  } else {
    return 2;
  }
}
