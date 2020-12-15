import("./dynamic_import/b.js").catch(() => {
  console.log("caught import error from b.js");
});

import("./dynamic_import/c.js").catch(() => {
  console.log("caught import error from c.js");
});
