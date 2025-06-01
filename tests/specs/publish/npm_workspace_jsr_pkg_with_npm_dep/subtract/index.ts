// test using a pkg.json dep and a workspace dep
import * as addDep from "add-dep";
import * as addWorkspaceDep from "add";

export function subtract(a: number, b: number): number {
  return addWorkspaceDep.add(addDep.add(a, -b), 1 - 1);
}
