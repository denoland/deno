import { printHello2, returnsFoo } from "mod2";

export function returnsHi(): string {
  return "Hi";
}

export function returnsFoo2(): string {
  return returnsFoo();
}

export function printHello3() {
  printHello2();
}

export function throwsError() {
  throw Error("exception from mod1");
}
