"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.serialize = exports.parse = void 0;
const parse = (cookie) => {
    const pairs = cookie.split(/;\s*/g);
    const parsedCookie = {};
    for (let i = 0, len = pairs.length; i < len; i++) {
        const pair = pairs[i].split(/\s*=\s*([^\s]+)/);
        parsedCookie[pair[0]] = decodeURIComponent(pair[1]);
    }
    return parsedCookie;
};
exports.parse = parse;
const serialize = (name, value, opt = {}) => {
    value = encodeURIComponent(value);
    let cookie = `${name}=${value}`;
    if (opt.maxAge) {
        cookie += `; Max-Age=${Math.floor(opt.maxAge)}`;
    }
    if (opt.domain) {
        cookie += '; Domain=' + opt.domain;
    }
    if (opt.path) {
        cookie += '; Path=' + opt.path;
    }
    if (opt.expires) {
        cookie += '; Expires=' + opt.expires.toUTCString();
    }
    if (opt.httpOnly) {
        cookie += '; HttpOnly';
    }
    if (opt.secure) {
        cookie += '; Secure';
    }
    if (opt.sameSite) {
        cookie += `; SameSite=${opt.sameSite}`;
    }
    return cookie;
};
exports.serialize = serialize;
