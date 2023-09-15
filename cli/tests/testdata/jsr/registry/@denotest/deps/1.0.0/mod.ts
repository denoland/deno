import { Other } from "jsr:@denotest/module_graph@1/other.ts";
import version from "jsr:@denotest/no_module_graph@^0.1/mod.ts";

export default {
  version,
  other: new Other(),
};
