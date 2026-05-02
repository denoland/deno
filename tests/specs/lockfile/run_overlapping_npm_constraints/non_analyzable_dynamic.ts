import { add } from "npm:@denotest/add@*";

function nonAnalyzable() {
  return "npm:@denotest/add@0.5.0";
}

// npm deduplication does not happen for dynamic because
// the `npm:@denotest/add@1.0.0` module has already been
// executed
const { sum } = await import(nonAnalyzable());

console.log(sum(1, 2));
console.log(add(1, 2));
