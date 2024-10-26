import { Other } from "jsr:@denotest/module-graph@1/other";
import version from "jsr:@denotest/no-module-graph@^0.1";

export default {
  version,
  other: new Other(),
};
