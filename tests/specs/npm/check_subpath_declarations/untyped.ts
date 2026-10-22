import { value } from "npm:@denotest/subpath-declarations@1.0.0/untyped";
import { value as root } from "npm:@denotest/subpath-declarations@1.0.0";
// An exported JS-only subpath retains Deno's ambient-any behavior.
const unchecked: number = value;
const typed: "typed" = root;
