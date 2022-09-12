import defaultExport, {
  myValue,
} from "npm:@denotest/cjs-esmodule-no-default-export";

// both should work since there's no default export
console.log(myValue);
console.log(defaultExport.myValue);
