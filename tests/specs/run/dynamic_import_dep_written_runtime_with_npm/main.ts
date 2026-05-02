Deno.writeTextFileSync(
  "./b.ts",
  `
import { add } from "npm:@denotest/add";
console.log(add(1, 2));
`,
);

console.log("Loading...");
await import("./a.ts");
