"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.cors = void 0;
const cors = (options) => {
    const defaults = {
        origin: '*',
        allowMethods: ['GET', 'HEAD', 'PUT', 'POST', 'DELETE', 'PATCH'],
        allowHeaders: [],
        exposeHeaders: [],
    };
    const opts = {
        ...defaults,
        ...options,
    };
    return async (c, next) => {
        await next();
        function set(key, value) {
            c.res.headers.append(key, value);
        }
        set('Access-Control-Allow-Origin', opts.origin);
        // Suppose the server sends a response with an Access-Control-Allow-Origin value with an explicit origin (rather than the "*" wildcard).
        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin
        if (opts.origin !== '*') {
            set('Vary', 'Origin');
        }
        if (opts.credentials) {
            set('Access-Control-Allow-Credentials', 'true');
        }
        if (opts.exposeHeaders?.length) {
            set('Access-Control-Expose-Headers', opts.exposeHeaders.join(','));
        }
        if (c.req.method === 'OPTIONS') {
            // Preflight
            if (opts.maxAge != null) {
                set('Access-Control-Max-Age', opts.maxAge.toString());
            }
            if (opts.allowMethods?.length) {
                set('Access-Control-Allow-Methods', opts.allowMethods.join(','));
            }
            let headers = opts.allowHeaders;
            if (!headers?.length) {
                const requestHeaders = c.req.headers.get('Access-Control-Request-Headers');
                if (requestHeaders) {
                    headers = requestHeaders.split(/\s*,\s*/);
                }
            }
            if (headers?.length) {
                set('Access-Control-Allow-Headers', headers.join(','));
                set('Vary', 'Access-Control-Request-Headers');
            }
            c.res.headers.delete('Content-Length');
            c.res.headers.delete('Content-Type');
            c.res = new Response(null, {
                headers: c.res.headers,
                status: 204,
                statusText: c.res.statusText,
            });
        }
    };
};
exports.cors = cors;
