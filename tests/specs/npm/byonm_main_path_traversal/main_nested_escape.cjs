const sub = require("nested-pkg/evil-sub");
console.log("ERROR: should not have loaded:", JSON.stringify(sub));
