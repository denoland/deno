import { double } from "./import_with_dep_helper.ts";

export function quadruple(n: number): number {
  return double(double(n));
}
