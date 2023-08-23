import codeBlockWriter from "codeBlockWriter";
import * as tsMorph from "tsMorph";
import * as assert from "@std/assert/mod.ts";

console.log(" > FOO > codeBlockWriter > ", codeBlockWriter);
console.log(" > FOO > tsMorph > ", tsMorph);
console.log(" > FOO > @std/assert > ", assert.assertEquals(1, 1));

export function foo() {
}
