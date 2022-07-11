'use strict';

const isLinux = () => process.platform === 'linux';

let report = null;
const getReport = () => {
  if (!report) {
    /* istanbul ignore next */
    report = isLinux() && process.report
      ? process.report.getReport()
      : {};
  }
  return report;
};

module.exports = { isLinux, getReport };
