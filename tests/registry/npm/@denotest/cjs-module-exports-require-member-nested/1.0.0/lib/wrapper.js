// Member-shape re-export reached through the entry module's normal
// re-export. The wrapper's named exports should be limited to
// properties of `safe` in `./inner.js`.
module.exports = require("./inner.js").safe;
