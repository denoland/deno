import { A } from "./A.ts";
import { thing } from "./common.ts";

export function C(): void {
  if (A != null) {
    thing();
  }
}
