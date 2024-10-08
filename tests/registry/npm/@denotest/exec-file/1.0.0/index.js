const child_process = require('child_process');
const path = require('path');

const execArgs = [path.join(__dirname, "exec-child.js")]
const buf = child_process.execFileSync(process.execPath, execArgs);
console.log(buf.toString().trim());
