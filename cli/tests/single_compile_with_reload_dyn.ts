import { printHello3, returnsFoo2, returnsHi } from "./subdir/mod1.ts";

printHello3();

if (returnsHi() !== "Hi") {
  throw Error("Unexpected");
}

if (returnsFoo2() !== "Foo") {
  throw Error("Unexpected");
}
