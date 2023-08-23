import codeBlockWriter from "codeBlockWriter";
import * as colors from "@std/fmt/colors.ts";
import { foo } from "deno:@foo/foo/foo.js";

console.log(">> BAR > codeBlockWriter > ", codeBlockWriter);
console.log(">> BAR > @std/fmt/colors > ", colors.red("Hello!"));
console.log(">> BAR > foo > ", foo);

export function bar() {
}
