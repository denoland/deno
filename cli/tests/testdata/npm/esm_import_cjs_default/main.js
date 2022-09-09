import defaultExport1 from "npm:@denotest/cjs-default-export";
import defaultExport2 from "npm:@denotest/esm-import-cjs-default";

console.log(defaultExport1());
console.log(defaultExport2());
