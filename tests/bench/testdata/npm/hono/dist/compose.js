"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.compose = void 0;
const context_1 = require("./context");
// Based on the code in the MIT licensed `koa-compose` package.
const compose = (middleware, onError, onNotFound) => {
    const middlewareLength = middleware.length;
    return (context, next) => {
        let index = -1;
        return dispatch(0);
        async function dispatch(i) {
            if (i <= index) {
                throw new Error('next() called multiple times');
            }
            let handler = middleware[i];
            index = i;
            if (i === middlewareLength && next)
                handler = next;
            if (!handler) {
                if (context instanceof context_1.HonoContext && context.finalized === false && onNotFound) {
                    context.res = await onNotFound(context);
                }
                return context;
            }
            let res;
            let isError = false;
            try {
                const tmp = handler(context, () => dispatch(i + 1));
                res = tmp instanceof Promise ? await tmp : tmp;
            }
            catch (err) {
                if (context instanceof context_1.HonoContext && onError) {
                    if (err instanceof Error) {
                        isError = true;
                        res = onError(err, context);
                    }
                }
                if (!res) {
                    throw err;
                }
            }
            if (res && context instanceof context_1.HonoContext && (!context.finalized || isError)) {
                context.res = res;
            }
            return context;
        }
    };
};
exports.compose = compose;
