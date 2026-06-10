"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.logger = void 0;
const url_1 = require("../../utils/url");
var LogPrefix;
(function (LogPrefix) {
    LogPrefix["Outgoing"] = "-->";
    LogPrefix["Incoming"] = "<--";
    LogPrefix["Error"] = "xxx";
})(LogPrefix || (LogPrefix = {}));
const humanize = (times) => {
    const [delimiter, separator] = [',', '.'];
    const orderTimes = times.map((v) => v.replace(/(\d)(?=(\d\d\d)+(?!\d))/g, '$1' + delimiter));
    return orderTimes.join(separator);
};
const time = (start) => {
    const delta = Date.now() - start;
    return humanize([delta < 1000 ? delta + 'ms' : Math.round(delta / 1000) + 's']);
};
const colorStatus = (status) => {
    const out = {
        7: `\x1b[35m${status}\x1b[0m`,
        5: `\x1b[31m${status}\x1b[0m`,
        4: `\x1b[33m${status}\x1b[0m`,
        3: `\x1b[36m${status}\x1b[0m`,
        2: `\x1b[32m${status}\x1b[0m`,
        1: `\x1b[32m${status}\x1b[0m`,
        0: `\x1b[33m${status}\x1b[0m`,
    };
    const calculateStatus = (status / 100) | 0;
    return out[calculateStatus];
};
function log(fn, prefix, method, path, status = 0, elapsed) {
    const out = prefix === LogPrefix.Incoming
        ? `  ${prefix} ${method} ${path}`
        : `  ${prefix} ${method} ${path} ${colorStatus(status)} ${elapsed}`;
    fn(out);
}
const logger = (fn = console.log) => {
    return async (c, next) => {
        const { method } = c.req;
        const path = (0, url_1.getPathFromURL)(c.req.url);
        log(fn, LogPrefix.Incoming, method, path);
        const start = Date.now();
        await next();
        log(fn, LogPrefix.Outgoing, method, path, c.res.status, time(start));
    };
};
exports.logger = logger;
