"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Node = void 0;
const LABEL_REG_EXP_STR = '[^/]+';
const ONLY_WILDCARD_REG_EXP_STR = '.*';
const TAIL_WILDCARD_REG_EXP_STR = '(?:|/.*)';
/**
 * Sort order:
 * 1. literal
 * 2. special pattern (e.g. :label{[0-9]+})
 * 3. common label pattern (e.g. :label)
 * 4. wildcard
 */
function compareKey(a, b) {
    if (a.length === 1) {
        return b.length === 1 ? (a < b ? -1 : 1) : -1;
    }
    if (b.length === 1) {
        return 1;
    }
    // wildcard
    if (a === ONLY_WILDCARD_REG_EXP_STR || a === TAIL_WILDCARD_REG_EXP_STR) {
        return 1;
    }
    else if (b === ONLY_WILDCARD_REG_EXP_STR || b === TAIL_WILDCARD_REG_EXP_STR) {
        return -1;
    }
    // label
    if (a === LABEL_REG_EXP_STR) {
        return 1;
    }
    else if (b === LABEL_REG_EXP_STR) {
        return -1;
    }
    return a.length === b.length ? (a < b ? -1 : 1) : b.length - a.length;
}
class Node {
    constructor({ reverse } = { reverse: false }) {
        this.children = {};
        this.reverse = reverse;
    }
    newChildNode() {
        return new Node({ reverse: this.reverse });
    }
    insert(tokens, index, paramMap, context) {
        var _a;
        if (tokens.length === 0) {
            this.index = index;
            return;
        }
        const [token, ...restTokens] = tokens;
        const pattern = token === '*'
            ? restTokens.length === 0
                ? ['', '', ONLY_WILDCARD_REG_EXP_STR] // '*' matches to all the trailing paths
                : ['', '', LABEL_REG_EXP_STR]
            : token === '/*'
                ? ['', '', TAIL_WILDCARD_REG_EXP_STR] // '/path/to/*' is /\/path\/to(?:|/.*)$
                : token.match(/^\:([^\{\}]+)(?:\{(.+)\})?$/);
        let node;
        if (pattern) {
            const name = pattern[1];
            const regexpStr = pattern[2] || LABEL_REG_EXP_STR;
            node = this.children[regexpStr];
            if (!node) {
                node = this.children[regexpStr] = this.newChildNode();
                if (name !== '') {
                    node.varIndex = context.varIndex++;
                }
            }
            if (name !== '') {
                paramMap.push([name, node.varIndex]);
            }
        }
        else {
            node = (_a = this.children)[token] || (_a[token] = this.newChildNode());
        }
        node.insert(restTokens, index, paramMap, context);
    }
    buildRegExpStr() {
        let childKeys = Object.keys(this.children).sort(compareKey);
        if (this.reverse) {
            childKeys = childKeys.reverse();
        }
        const strList = childKeys.map((k) => {
            const c = this.children[k];
            return (typeof c.varIndex === 'number' ? `(${k})@${c.varIndex}` : k) + c.buildRegExpStr();
        });
        if (typeof this.index === 'number') {
            strList.unshift(`#${this.index}`);
        }
        if (strList.length === 0) {
            return '';
        }
        if (strList.length === 1) {
            return strList[0];
        }
        return '(?:' + strList.join('|') + ')';
    }
}
exports.Node = Node;
