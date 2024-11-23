import defaultImport, * as namespaceImport from "npm:@denotest/cjs-module-export-assignment-number";

const testDefault: 5 = defaultImport;
console.log(testDefault);
const testNamespace: 5 = namespaceImport.default;
console.log(testNamespace);
console.log(namespaceImport);
