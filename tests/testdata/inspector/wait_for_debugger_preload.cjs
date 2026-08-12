const inspector = require("node:inspector");
inspector.open(0);
inspector.waitForDebugger();
