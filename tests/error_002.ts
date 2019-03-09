import { throwsError } from "./subdir/mod1.ts";

function foo(): void {
  throwsError();
}

foo();
