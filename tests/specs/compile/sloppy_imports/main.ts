import "./hello"; // no extension, resolves to hello.ts
import { greet } from "./greet.js"; // .js specifier resolves to greet.ts
import { fromDir } from "./subdir"; // directory resolves to subdir/index.ts

console.log(greet());
console.log(fromDir);

const { dyn } = await import("./dynamic"); // dynamic import, no extension
console.log(dyn);
