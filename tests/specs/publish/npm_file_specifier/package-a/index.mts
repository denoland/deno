import * as packageB from "package-b";

export function add(a: number, b: number): number {
  return packageB.add(a, b);
}
