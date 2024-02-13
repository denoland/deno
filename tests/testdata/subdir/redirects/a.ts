import { createA } from "./b.ts";

export class A {
  private _a = "a";
}

export function start(): A {
  return createA();
}
