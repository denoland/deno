// The npm registry these tests run against is served from loopback, so a
// `--deny-import` rule covering loopback must stop the registry request the
// same way it stops a remote `import`.
import { setValue } from "npm:@denotest/esm-basic";

console.log(typeof setValue);
