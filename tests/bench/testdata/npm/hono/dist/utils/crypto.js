"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.createHash = exports.md5 = exports.sha1 = exports.sha256 = void 0;
const sha256 = async (data) => {
    const algorithm = { name: 'SHA-256', alias: 'sha256' };
    const hash = await (0, exports.createHash)(data, algorithm);
    return hash;
};
exports.sha256 = sha256;
const sha1 = async (data) => {
    const algorithm = { name: 'SHA-1', alias: 'sha1' };
    const hash = await (0, exports.createHash)(data, algorithm);
    return hash;
};
exports.sha1 = sha1;
const md5 = async (data) => {
    const algorithm = { name: 'MD5', alias: 'md5' };
    const hash = await (0, exports.createHash)(data, algorithm);
    return hash;
};
exports.md5 = md5;
const createHash = async (data, algorithm) => {
    let sourceBuffer;
    if (data instanceof ReadableStream) {
        let body = '';
        const reader = data.getReader();
        await reader?.read().then(async (chuck) => {
            const value = await (0, exports.createHash)(chuck.value || '', algorithm);
            body += value;
        });
        return body;
    }
    if (ArrayBuffer.isView(data) || data instanceof ArrayBuffer) {
        sourceBuffer = data;
    }
    else {
        if (typeof data === 'object') {
            data = JSON.stringify(data);
        }
        sourceBuffer = new TextEncoder().encode(String(data));
    }
    if (crypto && crypto.subtle) {
        const buffer = await crypto.subtle.digest({
            name: algorithm.name,
        }, sourceBuffer);
        const hash = Array.prototype.map
            .call(new Uint8Array(buffer), (x) => ('00' + x.toString(16)).slice(-2))
            .join('');
        return hash;
    }
    return null;
};
exports.createHash = createHash;
