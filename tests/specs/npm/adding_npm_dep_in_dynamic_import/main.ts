import { add } from "npm:@denotest/add";

console.log(add(1, 2));

const fileName = "other.ts";
const specifier = "./" + fileName; // non-analyzable
const { subtract } = await import(specifier);
console.log(subtract(3, 2));
