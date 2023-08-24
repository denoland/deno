import codeBlockWriter from "codeBlockWriter";
import * as tsMorph from "tsMorph";
import * as assert from "@std/assert/mod.ts";

console.log(">> FOO > codeBlockWriter >");
console.log(codeBlockWriter);
console.log();
console.log(">> FOO > tsMorph >");
console.log(tsMorph.SetAccessorDeclaration);
console.log();
console.log(">> FOO > @std/assert >");
console.log(assert.assertEquals);
console.log();

export function foo() {
}
