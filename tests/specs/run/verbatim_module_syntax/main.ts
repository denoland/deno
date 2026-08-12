// `Foo` is only referenced as a type. Without verbatimModuleSyntax the
// transpiler elides the value import (and the side-effecting "dep loaded"
// console.log never runs). With verbatimModuleSyntax it must be preserved.
import { Foo } from "./dep.ts";

const _x: Foo | null = null;
console.log("main done");
