import { Other } from "deno:@denotest/module_graph@1/other.ts";
import version from "deno:@denotest/no_module_graph@^0.1/mod.ts";

export default {
  version,
  other: new Other(),
};
