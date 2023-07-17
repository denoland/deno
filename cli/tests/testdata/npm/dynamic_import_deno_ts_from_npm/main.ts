import { dynamicImport } from "npm:@denotest/dynamic-import";

const { add } = await dynamicImport(new URL("./add.ts", import.meta.url));
console.log(add(1, 2));
const { subtract } = await dynamicImport(
  new URL("./subtract.mts", import.meta.url),
);
console.log(subtract(1, 2));
