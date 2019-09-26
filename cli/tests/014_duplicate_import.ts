// with all the imports of the same module, the module should only be
// instantiated once
import "./subdir/auto_print_hello.ts";

import "./subdir/auto_print_hello.ts";

(async (): Promise<void> => {
  await import("./subdir/auto_print_hello.ts");
})();
