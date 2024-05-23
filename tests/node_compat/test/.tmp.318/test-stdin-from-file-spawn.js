
'use strict';
const { spawn } = require('child_process');
// Reference the object to invoke the getter
process.stdin;
setTimeout(() => {
  let ok = false;
  const child = spawn(process.env.SHELL || '/bin/sh',
    [], { stdio: ['inherit', 'pipe'] });
  child.stdout.on('data', () => {
    ok = true;
  });
  child.on('close', () => {
    process.exit(ok ? 0 : -1);
  });
}, 100);
