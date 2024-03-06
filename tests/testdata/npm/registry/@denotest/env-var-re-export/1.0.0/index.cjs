if (process.env.NODE_ENV === 'production') {
  module.exports = require('./prod.cjs');
} else {
  module.exports = require('./dev.cjs');
}
