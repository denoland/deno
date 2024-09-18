"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.mergePath = exports.isAbsoluteURL = exports.getPathFromURL = exports.getPattern = exports.splitPath = void 0;
const URL_REGEXP = /^https?:\/\/[a-zA-Z0-9\-\.:]+(\/?[^?#]*)/;
const splitPath = (path) => {
    const paths = path.split(/\//); // faster than path.split('/')
    if (paths[0] === '') {
        paths.shift();
    }
    return paths;
};
exports.splitPath = splitPath;
const patternCache = {};
const getPattern = (label) => {
    // *            => wildcard
    // :id{[0-9]+}  => ([0-9]+)
    // :id          => (.+)
    //const name = ''
    if (label === '*') {
        return '*';
    }
    const match = label.match(/^\:([^\{\}]+)(?:\{(.+)\})?$/);
    if (match) {
        if (!patternCache[label]) {
            if (match[2]) {
                patternCache[label] = [label, match[1], new RegExp('^' + match[2] + '$')];
            }
            else {
                patternCache[label] = [label, match[1], true];
            }
        }
        return patternCache[label];
    }
    return null;
};
exports.getPattern = getPattern;
const getPathFromURL = (url, strict = true) => {
    const queryIndex = url.indexOf('?');
    const result = url.substring(url.indexOf('/', 8), queryIndex === -1 ? url.length : queryIndex);
    // if strict routing is false => `/hello/hey/` and `/hello/hey` are treated the same
    // default is true
    if (strict === false && result.endsWith('/')) {
        return result.slice(0, -1);
    }
    return result;
};
exports.getPathFromURL = getPathFromURL;
const isAbsoluteURL = (url) => {
    const match = url.match(URL_REGEXP);
    if (match) {
        return true;
    }
    return false;
};
exports.isAbsoluteURL = isAbsoluteURL;
const mergePath = (...paths) => {
    let p = '';
    let endsWithSlash = false;
    for (let path of paths) {
        /* ['/hey/','/say'] => ['/hey', '/say'] */
        if (p.endsWith('/')) {
            p = p.slice(0, -1);
            endsWithSlash = true;
        }
        /* ['/hey','say'] => ['/hey', '/say'] */
        if (!path.startsWith('/')) {
            path = `/${path}`;
        }
        /* ['/hey/', '/'] => `/hey/` */
        if (path === '/' && endsWithSlash) {
            p = `${p}/`;
        }
        else if (path !== '/') {
            p = `${p}${path}`;
        }
        /* ['/', '/'] => `/` */
        if (path === '/' && p === '') {
            p = '/';
        }
    }
    return p;
};
exports.mergePath = mergePath;
