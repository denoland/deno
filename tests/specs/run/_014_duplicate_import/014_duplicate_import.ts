// with all the imports of the same module, the module should only be
// instantiated once
import "./auto_print_hello.ts";

import "./auto_print_hello.ts";

(async () => {
  await import("./auto_print_hello.ts");
})();
