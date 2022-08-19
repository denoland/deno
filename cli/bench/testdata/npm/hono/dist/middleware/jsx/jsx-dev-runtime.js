"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.jsxDEV = void 0;
const _1 = require(".");
function jsxDEV(tag, props) {
    const children = props.children ?? [];
    delete props['children'];
    return (0, _1.jsx)(tag, props, children);
}
exports.jsxDEV = jsxDEV;
