"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.html = exports.raw = void 0;
const html_1 = require("../../utils/html");
const raw = (value) => {
    const escapedString = new String(value);
    escapedString.isEscaped = true;
    return escapedString;
};
exports.raw = raw;
const html = (strings, ...values) => {
    const buffer = [''];
    for (let i = 0, len = strings.length - 1; i < len; i++) {
        buffer[0] += strings[i];
        const children = values[i] instanceof Array ? values[i].flat(Infinity) : [values[i]];
        for (let i = 0, len = children.length; i < len; i++) {
            const child = children[i];
            if (typeof child === 'string') {
                (0, html_1.escapeToBuffer)(child, buffer);
            }
            else if (typeof child === 'boolean' || child === null || child === undefined) {
                continue;
            }
            else if ((typeof child === 'object' && child.isEscaped) ||
                typeof child === 'number') {
                buffer[0] += child;
            }
            else {
                (0, html_1.escapeToBuffer)(child.toString(), buffer);
            }
        }
    }
    buffer[0] += strings[strings.length - 1];
    return (0, exports.raw)(buffer[0]);
};
exports.html = html;
