import defaultImport, { getValue } from "npm:@denotest/cjs-this-in-exports";

console.log(defaultImport.getValue());

// This will throw because it's lost its context.
// (same thing occurs with Node's cjs -> esm translation)
getValue();
