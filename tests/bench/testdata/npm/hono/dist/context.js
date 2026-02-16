"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.HonoContext = void 0;
const cookie_1 = require("./utils/cookie");
const url_1 = require("./utils/url");
class HonoContext {
    constructor(req, env = undefined, executionCtx = undefined, notFoundHandler = () => new Response()) {
        this._status = 200;
        this._pretty = false;
        this._prettySpace = 2;
        this._executionCtx = executionCtx;
        this.req = req;
        this.env = env ? env : {};
        this.notFoundHandler = notFoundHandler;
        this.finalized = false;
    }
    get event() {
        if (this._executionCtx instanceof FetchEvent) {
            return this._executionCtx;
        }
        else {
            throw Error('This context has no FetchEvent');
        }
    }
    get executionCtx() {
        if (this._executionCtx) {
            return this._executionCtx;
        }
        else {
            throw Error('This context has no ExecutionContext');
        }
    }
    get res() {
        return (this._res || (this._res = new Response()));
    }
    set res(_res) {
        this._res = _res;
        this.finalized = true;
    }
    header(name, value) {
        this._headers || (this._headers = {});
        this._headers[name.toLowerCase()] = value;
        if (this.finalized) {
            this.res.headers.set(name, value);
        }
    }
    status(status) {
        this._status = status;
    }
    set(key, value) {
        this._map || (this._map = {});
        this._map[key] = value;
    }
    get(key) {
        if (!this._map) {
            return undefined;
        }
        return this._map[key];
    }
    pretty(prettyJSON, space = 2) {
        this._pretty = prettyJSON;
        this._prettySpace = space;
    }
    newResponse(data, status, headers = {}) {
        const _headers = { ...this._headers };
        if (this._res) {
            this._res.headers.forEach((v, k) => {
                _headers[k] = v;
            });
        }
        return new Response(data, {
            status: status || this._status || 200,
            headers: { ..._headers, ...headers },
        });
    }
    body(data, status = this._status, headers = {}) {
        return this.newResponse(data, status, headers);
    }
    text(text, status = this._status, headers = {}) {
        headers['content-type'] = 'text/plain; charset=UTF-8';
        return this.body(text, status, headers);
    }
    json(object, status = this._status, headers = {}) {
        const body = this._pretty
            ? JSON.stringify(object, null, this._prettySpace)
            : JSON.stringify(object);
        headers['content-type'] = 'application/json; charset=UTF-8';
        return this.body(body, status, headers);
    }
    html(html, status = this._status, headers = {}) {
        headers['content-type'] = 'text/html; charset=UTF-8';
        return this.body(html, status, headers);
    }
    redirect(location, status = 302) {
        if (!(0, url_1.isAbsoluteURL)(location)) {
            const url = new URL(this.req.url);
            url.pathname = location;
            location = url.toString();
        }
        return this.newResponse(null, status, {
            Location: location,
        });
    }
    cookie(name, value, opt) {
        const cookie = (0, cookie_1.serialize)(name, value, opt);
        this.header('set-cookie', cookie);
    }
    notFound() {
        return this.notFoundHandler(this);
    }
}
exports.HonoContext = HonoContext;
