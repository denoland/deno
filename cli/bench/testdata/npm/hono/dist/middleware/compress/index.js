"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.compress = void 0;
const compress = (options) => {
    return async (ctx, next) => {
        await next();
        const accepted = ctx.req.headers.get('Accept-Encoding');
        const pattern = options?.encoding ?? /gzip|deflate/;
        const match = accepted?.match(pattern);
        if (!accepted || !match || !ctx.res.body) {
            return;
        }
        const encoding = match[0];
        const stream = new CompressionStream(encoding);
        ctx.res = new Response(ctx.res.body.pipeThrough(stream), ctx.res.clone());
        ctx.res.headers.set('Content-Encoding', encoding);
    };
};
exports.compress = compress;
