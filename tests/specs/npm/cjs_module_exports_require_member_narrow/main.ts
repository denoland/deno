// Properties statically attached to the re-exported member ARE
// advertised by the wrapper. Confirms the member-shape fallback is
// still active for safe shapes.
import safe, {
  attached,
} from "npm:@denotest/cjs-module-exports-require-member-narrow";

console.log(safe());
console.log(attached());
