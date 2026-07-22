import { getValue } from "npm:@denotest/require-added-nm-folder";

Deno.mkdirSync("./node_modules/.other-package");
Deno.writeTextFileSync("./node_modules/.other-package/package.json", "{}");
Deno.writeTextFileSync(
  "./node_modules/.other-package/index.js",
  "exports.get = () => 5;",
);

console.log(getValue());
