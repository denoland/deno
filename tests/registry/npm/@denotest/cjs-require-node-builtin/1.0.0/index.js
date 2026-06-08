const util = require("util");
const stream = require("stream");

module.exports = {
  format: util.format,
  readableType: typeof stream.Readable,
};
