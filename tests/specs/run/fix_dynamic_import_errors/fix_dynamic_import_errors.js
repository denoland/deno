import("./b.js").catch(() => {
  console.log("caught import error from b.js");
});

import("./c.js").catch(() => {
  console.log("caught import error from c.js");
});
