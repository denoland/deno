"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Fragment = exports.memo = exports.jsx = exports.JSXNode = void 0;
const html_1 = require("../../utils/html");
const emptyTags = [
    'area',
    'base',
    'br',
    'col',
    'embed',
    'hr',
    'img',
    'input',
    'keygen',
    'link',
    'meta',
    'param',
    'source',
    'track',
    'wbr',
];
const booleanAttributes = [
    'allowfullscreen',
    'async',
    'autofocus',
    'autoplay',
    'checked',
    'controls',
    'default',
    'defer',
    'disabled',
    'formnovalidate',
    'hidden',
    'inert',
    'ismap',
    'itemscope',
    'loop',
    'multiple',
    'muted',
    'nomodule',
    'novalidate',
    'open',
    'playsinline',
    'readonly',
    'required',
    'reversed',
    'selected',
];
const childrenToStringToBuffer = (children, buffer) => {
    for (let i = 0, len = children.length; i < len; i++) {
        const child = children[i];
        if (typeof child === 'string') {
            (0, html_1.escapeToBuffer)(child, buffer);
        }
        else if (typeof child === 'boolean' || child === null || child === undefined) {
            continue;
        }
        else if (child instanceof JSXNode) {
            child.toStringToBuffer(buffer);
        }
        else if (typeof child === 'number' || child.isEscaped) {
            buffer[0] += child;
        }
        else {
            // `child` type is `Child[]`, so stringify recursively
            childrenToStringToBuffer(child, buffer);
        }
    }
};
class JSXNode {
    constructor(tag, props, children) {
        this.isEscaped = true;
        this.tag = tag;
        this.props = props;
        this.children = children;
    }
    toString() {
        const buffer = [''];
        this.toStringToBuffer(buffer);
        return buffer[0];
    }
    toStringToBuffer(buffer) {
        const tag = this.tag;
        const props = this.props;
        let { children } = this;
        buffer[0] += `<${tag}`;
        const propsKeys = Object.keys(props || {});
        for (let i = 0, len = propsKeys.length; i < len; i++) {
            const v = props[propsKeys[i]];
            if (typeof v === 'string') {
                buffer[0] += ` ${propsKeys[i]}="`;
                (0, html_1.escapeToBuffer)(v, buffer);
                buffer[0] += '"';
            }
            else if (typeof v === 'number') {
                buffer[0] += ` ${propsKeys[i]}="${v}"`;
            }
            else if (v === null || v === undefined) {
                // Do nothing
            }
            else if (typeof v === 'boolean' && booleanAttributes.includes(propsKeys[i])) {
                if (v) {
                    buffer[0] += ` ${propsKeys[i]}=""`;
                }
            }
            else if (propsKeys[i] === 'dangerouslySetInnerHTML') {
                if (children.length > 0) {
                    throw 'Can only set one of `children` or `props.dangerouslySetInnerHTML`.';
                }
                const escapedString = new String(v.__html);
                escapedString.isEscaped = true;
                children = [escapedString];
            }
            else {
                buffer[0] += ` ${propsKeys[i]}="`;
                (0, html_1.escapeToBuffer)(v.toString(), buffer);
                buffer[0] += '"';
            }
        }
        if (emptyTags.includes(tag)) {
            buffer[0] += '/>';
            return;
        }
        buffer[0] += '>';
        childrenToStringToBuffer(children, buffer);
        buffer[0] += `</${tag}>`;
    }
}
exports.JSXNode = JSXNode;
class JSXFunctionNode extends JSXNode {
    toStringToBuffer(buffer) {
        const { children } = this;
        const res = this.tag.call(null, {
            ...this.props,
            children: children.length <= 1 ? children[0] : children,
        });
        if (res instanceof JSXNode) {
            res.toStringToBuffer(buffer);
        }
        else if (typeof res === 'number' || res.isEscaped) {
            buffer[0] += res;
        }
        else {
            (0, html_1.escapeToBuffer)(res, buffer);
        }
    }
}
class JSXFragmentNode extends JSXNode {
    toStringToBuffer(buffer) {
        childrenToStringToBuffer(this.children, buffer);
    }
}
const jsxFn = (tag, props, ...children) => {
    if (typeof tag === 'function') {
        return new JSXFunctionNode(tag, props, children);
    }
    else {
        return new JSXNode(tag, props, children);
    }
};
exports.jsx = jsxFn;
const shallowEqual = (a, b) => {
    if (a === b) {
        return true;
    }
    const aKeys = Object.keys(a);
    const bKeys = Object.keys(b);
    if (aKeys.length !== bKeys.length) {
        return false;
    }
    for (let i = 0, len = aKeys.length; i < len; i++) {
        if (a[aKeys[i]] !== b[aKeys[i]]) {
            return false;
        }
    }
    return true;
};
const memo = (component, propsAreEqual = shallowEqual) => {
    let computed = undefined;
    let prevProps = undefined;
    return ((props) => {
        if (prevProps && !propsAreEqual(prevProps, props)) {
            computed = undefined;
        }
        prevProps = props;
        return (computed || (computed = component(props)));
    });
};
exports.memo = memo;
const Fragment = (props) => {
    return new JSXFragmentNode('', {}, props.children || []);
};
exports.Fragment = Fragment;
