module.exports = {
  sayHi: () => 'Hi from node-lifecycle-scripts!'
};

const fs = require('fs');
const path = require('path');

fs.writeFileSync(path.join(process.env.INIT_CWD, 'install.txt'), 'Installed by @denotest/node-lifecycle-scripts!');


console.log('install.js', module.exports.sayHi());