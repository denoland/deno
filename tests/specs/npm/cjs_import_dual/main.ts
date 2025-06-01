import { getKind } from "npm:@denotest/cjs-import-dual@1";

const kind: "esm" = getKind(); // should cause a type error
console.log(kind);
