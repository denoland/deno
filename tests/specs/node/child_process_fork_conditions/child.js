const pkg = require("test-pkg");

// Send the result back to parent process
process.send({ type: pkg.type });
