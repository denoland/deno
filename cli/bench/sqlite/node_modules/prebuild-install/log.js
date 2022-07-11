const log = require('npmlog')
const fs = require('fs')
const path = require('path')

module.exports = function (rc, env) {
  log.heading = 'prebuild-install'

  if (rc.verbose) {
    log.level = 'verbose'
  } else {
    log.level = env.npm_config_loglevel || 'notice'
  }

  // Temporary workaround for npm 7 which swallows our output
  if (process.env.npm_config_prebuild_install_logfile) {
    const fp = path.resolve(process.env.npm_config_prebuild_install_logfile)

    log.on('log', function (msg) {
      // Only for tests, don't care about performance
      fs.appendFileSync(fp, [log.heading, msg.level, msg.prefix, msg.message].join(' ') + '\n')
    })
  }

  return log
}
