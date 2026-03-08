"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.prettyJSON = void 0;
const prettyJSON = (options = { space: 2 }) => {
    return async (c, next) => {
        const pretty = c.req.query('pretty') || c.req.query('pretty') === '' ? true : false;
        c.pretty(pretty, options.space);
        await next();
    };
};
exports.prettyJSON = prettyJSON;
