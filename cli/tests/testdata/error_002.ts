import { throwsError } from "./subdir/mod1.ts";

function foo() {
  throwsError();
}

foo();
