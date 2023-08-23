import codeBlockWriter from "codeBlockWriter";
import * as chain from "chain/file1.ts";
import * as colors from "@std/fmt/colors.ts";
// import { foo } from "deno:@foo/foo@1";

console.log(" > BAR > codeBlockWriter > ", codeBlockWriter);
console.log(" > BAR > chain > ", chain);
console.log(" > BAR > @std/fmt/colors > ", colors.red("Hello!"));
// console.log(" > BAR > foo > ", foo);

export function bar() {
}
