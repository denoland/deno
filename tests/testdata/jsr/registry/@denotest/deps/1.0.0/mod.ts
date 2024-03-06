import { Other } from "jsr:@denotest/module_graph@1/other";
import version from "jsr:@denotest/no_module_graph@^0.1";

export default {
  version,
  other: new Other(),
};
