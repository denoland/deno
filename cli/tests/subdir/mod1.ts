import { printHello2, returnsFoo } from "./subdir2/mod2.ts";

export function returnsHi(): string {
  return "Hi";
}

export function returnsFoo2(): string {
  return returnsFoo();
}

export function printHello3(): void {
  printHello2();
}

export function throwsError(): void {
  throw Error("exception from mod1");
}
