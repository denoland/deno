import { printHello } from "../print_hello.ts";

export function returnsFoo(): string {
  return "Foo";
}

export function printHello2() {
  printHello();
}
