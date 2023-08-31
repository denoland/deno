"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.basicAuth = void 0;
const buffer_1 = require("../../utils/buffer");
const encode_1 = require("../../utils/encode");
const CREDENTIALS_REGEXP = /^ *(?:[Bb][Aa][Ss][Ii][Cc]) +([A-Za-z0-9._~+/-]+=*) *$/;
const USER_PASS_REGEXP = /^([^:]*):(.*)$/;
const auth = (req) => {
    const match = CREDENTIALS_REGEXP.exec(req.headers.get('Authorization') || '');
    if (!match) {
        return undefined;
    }
    const userPass = USER_PASS_REGEXP.exec((0, encode_1.decodeBase64)(match[1]));
    if (!userPass) {
        return undefined;
    }
    return { username: userPass[1], password: userPass[2] };
};
const basicAuth = (options, ...users) => {
    if (!options) {
        throw new Error('basic auth middleware requires options for "username and password"');
    }
    if (!options.realm) {
        options.realm = 'Secure Area';
    }
    users.unshift({ username: options.username, password: options.password });
    return async (ctx, next) => {
        const requestUser = auth(ctx.req);
        if (requestUser) {
            for (const user of users) {
                const usernameEqual = await (0, buffer_1.timingSafeEqual)(user.username, requestUser.username, options.hashFunction);
                const passwordEqual = await (0, buffer_1.timingSafeEqual)(user.password, requestUser.password, options.hashFunction);
                if (usernameEqual && passwordEqual) {
                    // Authorized OK
                    await next();
                    return;
                }
            }
        }
        ctx.res = new Response('Unauthorized', {
            status: 401,
            headers: {
                'WWW-Authenticate': 'Basic realm="' + options.realm?.replace(/"/g, '\\"') + '"',
            },
        });
    };
};
exports.basicAuth = basicAuth;
