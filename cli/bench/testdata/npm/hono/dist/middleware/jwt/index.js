"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.jwt = void 0;
const jwt_1 = require("../../utils/jwt");
const jwt = (options) => {
    if (!options) {
        throw new Error('JWT auth middleware requires options for "secret');
    }
    if (!crypto.subtle || !crypto.subtle.importKey) {
        throw new Error('`crypto.subtle.importKey` is undefined. JWT auth middleware requires it.');
    }
    return async (ctx, next) => {
        const credentials = ctx.req.headers.get('Authorization');
        let token;
        if (credentials) {
            const parts = credentials.split(/\s+/);
            if (parts.length !== 2) {
                ctx.res = new Response('Unauthorized', {
                    status: 401,
                    headers: {
                        'WWW-Authenticate': `Bearer realm="${ctx.req.url}",error="invalid_request",error_description="invalid credentials structure"`,
                    },
                });
                return;
            }
            else {
                token = parts[1];
            }
        }
        else if (options.cookie) {
            token = ctx.req.cookie(options.cookie);
        }
        if (!token) {
            ctx.res = new Response('Unauthorized', {
                status: 401,
                headers: {
                    'WWW-Authenticate': `Bearer realm="${ctx.req.url}",error="invalid_request",error_description="no authorization included in request"`,
                },
            });
            return;
        }
        let authorized = false;
        let msg = '';
        try {
            authorized = await jwt_1.Jwt.verify(token, options.secret, options.alg);
        }
        catch (e) {
            msg = `${e}`;
        }
        if (!authorized) {
            ctx.res = new Response('Unauthorized', {
                status: 401,
                statusText: msg,
                headers: {
                    'WWW-Authenticate': `Bearer realm="${ctx.req.url}",error="invalid_token",error_description="token verification failure"`,
                },
            });
            return;
        }
        await next();
    };
};
exports.jwt = jwt;
