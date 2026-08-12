// The entry module re-exports a wrapper module whose body is a
// member-shape re-export. Properties statically attached to the
// re-exported member must still be advertised through the recursive
// re-export chain.
import safe, {
  attached,
} from "npm:@denotest/cjs-module-exports-require-member-nested";

console.log(safe());
console.log(attached());
