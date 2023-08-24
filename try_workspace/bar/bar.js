import codeBlockWriter from "codeBlockWriter";
import * as colors from "@std/fmt/colors.ts";
import { foo } from "foo/foo.js";

console.log(">> BAR > codeBlockWriter >");
console.log(codeBlockWriter);
console.log();
console.log(">> BAR > @std/fmt/colors >");
console.log(colors.red("Hello!"));
console.log();
console.log(">> BAR > foo >");
console.log(foo);
console.log();

export function bar() {
}
