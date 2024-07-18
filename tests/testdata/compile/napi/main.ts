import Module from "node:module";
import path from "node:path";

const mod = new Module("");

const filepath = Deno.args[0];

console.log(mod.require(filepath));
