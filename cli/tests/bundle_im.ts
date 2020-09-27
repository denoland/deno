import { printHello2, returnsFoo } from "mod2";

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
