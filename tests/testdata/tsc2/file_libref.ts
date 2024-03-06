// deno-lint-ignore-file
/// <reference no-default-lib="true"/>
/// <reference lib="dom" />
/// <reference lib="deno.ns" />

export const div = document.createElement("div");
div.innerHTML = `<span>Hello World!</span>`;
console.log(Deno.args);
