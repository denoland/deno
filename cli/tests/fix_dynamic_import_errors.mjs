import("./dynamic_import/b.mjs").catch(() => {
  console.log("caught import error from b.mjs");
});

import("./dynamic_import/c.mjs").catch(() => {
  console.log("caught import error from c.mjs");
});
