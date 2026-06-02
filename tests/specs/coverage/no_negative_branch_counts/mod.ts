export function loopWithBranch(items: number[]): number {
  let sum = 0;
  for (const item of items) {
    if (item > 0) {
      sum += item;
    } else {
      sum -= item;
    }
  }
  return sum;
}
