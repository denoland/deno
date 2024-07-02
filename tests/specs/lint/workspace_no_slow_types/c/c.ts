import { noReturnType } from "@scope/a";
import { hasReturnType } from "@scope/b";

export function myExport(): number {
  return noReturnType() + hasReturnType();
}
