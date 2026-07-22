"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.extendRequestPrototype = void 0;
const body_1 = require("./utils/body");
const cookie_1 = require("./utils/cookie");
function extendRequestPrototype() {
    if (!!Request.prototype.param) {
        // already extended
        return;
    }
    Request.prototype.param = function (key) {
        if (this.paramData) {
            if (key) {
                return this.paramData[key];
            }
            else {
                return this.paramData;
            }
        }
        return null;
    };
    Request.prototype.header = function (name) {
        if (name) {
            return this.headers.get(name);
        }
        else {
            const result = {};
            for (const [key, value] of this.headers) {
                result[key] = value;
            }
            return result;
        }
    };
    Request.prototype.query = function (key) {
        const url = new URL(this.url);
        if (key) {
            return url.searchParams.get(key);
        }
        else {
            const result = {};
            for (const key of url.searchParams.keys()) {
                result[key] = url.searchParams.get(key) || '';
            }
            return result;
        }
    };
    Request.prototype.queries = function (key) {
        const url = new URL(this.url);
        if (key) {
            return url.searchParams.getAll(key);
        }
        else {
            const result = {};
            for (const key of url.searchParams.keys()) {
                result[key] = url.searchParams.getAll(key);
            }
            return result;
        }
    };
    Request.prototype.cookie = function (key) {
        const cookie = this.headers.get('Cookie') || '';
        const obj = (0, cookie_1.parse)(cookie);
        if (key) {
            const value = obj[key];
            return value;
        }
        else {
            return obj;
        }
    };
    Request.prototype.parseBody = function () {
        if (!this.parsedBody) {
            this.parsedBody = (0, body_1.parseBody)(this);
        }
        return this.parsedBody;
    };
}
exports.extendRequestPrototype = extendRequestPrototype;
