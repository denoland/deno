"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Node = void 0;
const router_1 = require("../../router");
const url_1 = require("../../utils/url");
function findParam(node, name) {
    for (let i = 0, len = node.patterns.length; i < len; i++) {
        if (typeof node.patterns[i] === 'object' && node.patterns[i][1] === name) {
            return true;
        }
    }
    const nodes = Object.values(node.children);
    for (let i = 0, len = nodes.length; i < len; i++) {
        if (findParam(nodes[i], name)) {
            return true;
        }
    }
    return false;
}
class Node {
    constructor(method, handler, children) {
        this.order = 0;
        this.children = children || {};
        this.methods = [];
        this.name = '';
        if (method && handler) {
            const m = {};
            m[method] = { handler: handler, score: 0, name: this.name };
            this.methods = [m];
        }
        this.patterns = [];
        this.handlerSetCache = {};
    }
    insert(method, path, handler) {
        this.name = `${method} ${path}`;
        this.order = ++this.order;
        // eslint-disable-next-line @typescript-eslint/no-this-alias
        let curNode = this;
        const parts = (0, url_1.splitPath)(path);
        const parentPatterns = [];
        const errorMessage = (name) => {
            return `Duplicate param name, use another name instead of '${name}' - ${method} ${path} <--- '${name}'`;
        };
        for (let i = 0, len = parts.length; i < len; i++) {
            const p = parts[i];
            if (Object.keys(curNode.children).includes(p)) {
                parentPatterns.push(...curNode.patterns);
                curNode = curNode.children[p];
                continue;
            }
            curNode.children[p] = new Node();
            const pattern = (0, url_1.getPattern)(p);
            if (pattern) {
                if (typeof pattern === 'object') {
                    for (let j = 0, len = parentPatterns.length; j < len; j++) {
                        if (typeof parentPatterns[j] === 'object' && parentPatterns[j][1] === pattern[1]) {
                            throw new Error(errorMessage(pattern[1]));
                        }
                    }
                    if (Object.values(curNode.children).some((n) => findParam(n, pattern[1]))) {
                        throw new Error(errorMessage(pattern[1]));
                    }
                }
                curNode.patterns.push(pattern);
                parentPatterns.push(...curNode.patterns);
            }
            parentPatterns.push(...curNode.patterns);
            curNode = curNode.children[p];
        }
        if (!curNode.methods.length) {
            curNode.methods = [];
        }
        const m = {};
        const handlerSet = { handler: handler, name: this.name, score: this.order };
        m[method] = handlerSet;
        curNode.methods.push(m);
        return curNode;
    }
    getHandlerSets(node, method, wildcard) {
        var _a, _b;
        return ((_a = node.handlerSetCache)[_b = `${method}:${wildcard ? '1' : '0'}`] || (_a[_b] = (() => {
            const handlerSets = [];
            node.methods.map((m) => {
                const handlerSet = m[method] || m[router_1.METHOD_NAME_ALL];
                if (handlerSet !== undefined) {
                    const hs = { ...handlerSet };
                    handlerSets.push(hs);
                    return;
                }
            });
            return handlerSets;
        })()));
    }
    search(method, path) {
        const handlerSets = [];
        const params = {};
        // eslint-disable-next-line @typescript-eslint/no-this-alias
        const curNode = this;
        let curNodes = [curNode];
        const parts = (0, url_1.splitPath)(path);
        for (let i = 0, len = parts.length; i < len; i++) {
            const part = parts[i];
            const isLast = i === len - 1;
            const tempNodes = [];
            let matched = false;
            for (let j = 0, len2 = curNodes.length; j < len2; j++) {
                const node = curNodes[j];
                const nextNode = node.children[part];
                if (nextNode) {
                    if (isLast === true) {
                        // '/hello/*' => match '/hello'
                        if (nextNode.children['*']) {
                            handlerSets.push(...this.getHandlerSets(nextNode.children['*'], method, true));
                        }
                        handlerSets.push(...this.getHandlerSets(nextNode, method));
                        matched = true;
                    }
                    tempNodes.push(nextNode);
                }
                for (let k = 0, len3 = node.patterns.length; k < len3; k++) {
                    const pattern = node.patterns[k];
                    // Wildcard
                    // '/hello/*/foo' => match /hello/bar/foo
                    if (pattern === '*') {
                        const astNode = node.children['*'];
                        if (astNode) {
                            handlerSets.push(...this.getHandlerSets(astNode, method));
                            tempNodes.push(astNode);
                        }
                        continue;
                    }
                    if (part === '')
                        continue;
                    // Named match
                    // `/posts/:id` => match /posts/123
                    const [key, name, matcher] = pattern;
                    if (matcher === true || (matcher instanceof RegExp && matcher.test(part))) {
                        if (typeof key === 'string') {
                            if (isLast === true) {
                                handlerSets.push(...this.getHandlerSets(node.children[key], method));
                            }
                            tempNodes.push(node.children[key]);
                        }
                        // '/book/a'     => not-slug
                        // '/book/:slug' => slug
                        // GET /book/a   ~> no-slug, param['slug'] => undefined
                        // GET /book/foo ~> slug, param['slug'] => foo
                        if (typeof name === 'string' && !matched) {
                            params[name] = part;
                        }
                    }
                }
            }
            curNodes = tempNodes;
        }
        if (handlerSets.length <= 0)
            return null;
        const handlers = handlerSets
            .sort((a, b) => {
            return a.score - b.score;
        })
            .map((s) => {
            return s.handler;
        });
        return { handlers, params };
    }
}
exports.Node = Node;
