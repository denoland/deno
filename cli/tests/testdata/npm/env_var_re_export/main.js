import {
  devOnlyExport,
  getEnv,
  prodOnlyExport,
} from "npm:@denotest/env-var-re-export";
import { expect } from "npm:chai@4.3";

if (Deno.env.get("NODE_ENV") === "production") {
  expect(devOnlyExport).to.be.undefined;
  expect(prodOnlyExport()).to.equal(1);
} else {
  expect(devOnlyExport()).to.equal(1);
  expect(prodOnlyExport).to.be.undefined;
}

console.log(getEnv());
