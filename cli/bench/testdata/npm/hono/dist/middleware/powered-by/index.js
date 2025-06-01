"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.poweredBy = void 0;
const poweredBy = () => {
    return async (c, next) => {
        await next();
        c.res.headers.append('X-Powered-By', 'Hono');
    };
};
exports.poweredBy = poweredBy;
