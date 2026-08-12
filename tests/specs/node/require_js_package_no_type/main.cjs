const configure = require("./karma.conf.js");
const config = {
  set(value) {
    console.log(value.framework);
  },
};

configure(config);
