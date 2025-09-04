import * as fs from "node:fs";

fs.writeFileSync("./testbin.js", "#!/usr/bin/env node\nconsole.log('run testbin');");