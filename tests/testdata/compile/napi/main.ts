import Module from "node:module";

const mod = new Module("");

const filepath = Deno.args[0];

console.log(mod.require(filepath));
