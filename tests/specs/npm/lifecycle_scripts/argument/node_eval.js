import { value } from "npm:@denotest/node-eval-lifecycle";

if (value !== "node-eval-lifecycle") {
  throw new Error("unexpected package value");
}
