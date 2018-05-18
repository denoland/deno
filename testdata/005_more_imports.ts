import { returnsHi, returnsFoo2, printHello3 } from "./subdir/mod1.ts";

printHello3();

if (returnsHi() !== "Hi") {
  throw Error("Unexpected");
}

if (returnsFoo2() !== "Foo") {
  throw Error("Unexpected");
}
