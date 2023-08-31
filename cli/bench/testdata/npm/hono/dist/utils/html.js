"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.escapeToBuffer = void 0;
// The `escapeToBuffer` implementation is based on code from the MIT licensed `react-dom` package.
// https://github.com/facebook/react/blob/main/packages/react-dom/src/server/escapeTextForBrowser.js
const escapeRe = /[&<>"]/;
const escapeToBuffer = (str, buffer) => {
    const match = str.search(escapeRe);
    if (match === -1) {
        buffer[0] += str;
        return;
    }
    let escape;
    let index;
    let lastIndex = 0;
    for (index = match; index < str.length; index++) {
        switch (str.charCodeAt(index)) {
            case 34: // "
                escape = '&quot;';
                break;
            case 38: // &
                escape = '&amp;';
                break;
            case 60: // <
                escape = '&lt;';
                break;
            case 62: // >
                escape = '&gt;';
                break;
            default:
                continue;
        }
        buffer[0] += str.substring(lastIndex, index) + escape;
        lastIndex = index + 1;
    }
    buffer[0] += str.substring(lastIndex, index);
};
exports.escapeToBuffer = escapeToBuffer;
