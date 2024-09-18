"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Hono = void 0;
const compose_1 = require("./compose");
const context_1 = require("./context");
const request_1 = require("./request");
const router_1 = require("./router");
const trie_router_1 = require("./router/trie-router"); // Default Router
const url_1 = require("./utils/url");
const methods = ['get', 'post', 'put', 'delete', 'head', 'options', 'patch'];
function defineDynamicClass() {
    return class {
    };
}
class Hono extends defineDynamicClass() {
    constructor(init = {}) {
        super();
        this.router = new trie_router_1.TrieRouter();
        this.strict = true; // strict routing - default is true
        this._tempPath = '';
        this.path = '/';
        this.routes = [];
        this.notFoundHandler = (c) => {
            const message = '404 Not Found';
            return c.text(message, 404);
        };
        this.errorHandler = (err, c) => {
            console.error(`${err.stack || err.message}`);
            const message = 'Internal Server Error';
            return c.text(message, 500);
        };
        this.fetch = (request, env, executionCtx) => {
            return this.dispatch(request, executionCtx, env);
        };
        (0, request_1.extendRequestPrototype)();
        const allMethods = [...methods, router_1.METHOD_NAME_ALL_LOWERCASE];
        allMethods.map((method) => {
            this[method] = (args1, ...args) => {
                if (typeof args1 === 'string') {
                    this.path = args1;
                }
                else {
                    this.addRoute(method, this.path, args1);
                }
                args.map((handler) => {
                    if (typeof handler !== 'string') {
                        this.addRoute(method, this.path, handler);
                    }
                });
                return this;
            };
        });
        Object.assign(this, init);
    }
    route(path, app) {
        this._tempPath = path;
        if (app) {
            app.routes.map((r) => {
                this.addRoute(r.method, r.path, r.handler);
            });
            this._tempPath = '';
        }
        return this;
    }
    use(arg1, ...handlers) {
        if (typeof arg1 === 'string') {
            this.path = arg1;
        }
        else {
            handlers.unshift(arg1);
        }
        handlers.map((handler) => {
            this.addRoute(router_1.METHOD_NAME_ALL, this.path, handler);
        });
        return this;
    }
    onError(handler) {
        this.errorHandler = handler;
        return this;
    }
    notFound(handler) {
        this.notFoundHandler = handler;
        return this;
    }
    addRoute(method, path, handler) {
        method = method.toUpperCase();
        if (this._tempPath) {
            path = (0, url_1.mergePath)(this._tempPath, path);
        }
        this.router.add(method, path, handler);
        const r = { path: path, method: method, handler: handler };
        this.routes.push(r);
    }
    matchRoute(method, path) {
        return this.router.match(method, path);
    }
    async dispatch(request, eventOrExecutionCtx, env) {
        const path = (0, url_1.getPathFromURL)(request.url, this.strict);
        const method = request.method;
        const result = this.matchRoute(method, path);
        request.paramData = result?.params;
        const handlers = result ? result.handlers : [this.notFoundHandler];
        const c = new context_1.HonoContext(request, env, eventOrExecutionCtx, this.notFoundHandler);
        const composed = (0, compose_1.compose)(handlers, this.errorHandler, this.notFoundHandler);
        let context;
        try {
            context = await composed(c);
            if (!context.finalized) {
                throw new Error('Context is not finalized. You may forget returning Response object or `await next()`');
            }
        }
        catch (err) {
            if (err instanceof Error) {
                return this.errorHandler(err, c);
            }
            throw err;
        }
        return context.res;
    }
    handleEvent(event) {
        return this.dispatch(event.request, event);
    }
    request(input, requestInit) {
        const req = input instanceof Request ? input : new Request(input, requestInit);
        return this.dispatch(req);
    }
}
exports.Hono = Hono;
