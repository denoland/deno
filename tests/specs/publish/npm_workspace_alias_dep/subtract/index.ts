// `my-add` is a pnpm-style alias (`workspace:add@^`) to the `add` workspace
// member. On publish it should unfurl to the member's package, not the alias.
import { add } from "my-add";

export function subtract(a: number, b: number): number {
  return add(a, -b);
}
