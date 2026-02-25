"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseBody = void 0;
const parseBody = async (r) => {
    const contentType = r.headers.get('Content-Type') || '';
    if (contentType.includes('application/json')) {
        let body = {};
        try {
            body = await r.json();
        }
        catch { } // Do nothing
        return body;
    }
    else if (contentType.includes('application/text')) {
        return await r.text();
    }
    else if (contentType.startsWith('text')) {
        return await r.text();
    }
    else if (contentType.includes('form')) {
        const form = {};
        const data = [...(await r.formData())].reduce((acc, cur) => {
            acc[cur[0]] = cur[1];
            return acc;
        }, form);
        return data;
    }
    const arrayBuffer = await r.arrayBuffer();
    return arrayBuffer;
};
exports.parseBody = parseBody;
