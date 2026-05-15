// Test that __exportStar re-exports from a CJS package whose "main"
// field has no file extension are properly detected during static
// analysis for ESM wrapping.
import { MyClass } from "npm:@denotest/cjs-exportstar-reexport@1.0.0";

const instance = new MyClass();
console.log(instance.getValue());
