"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RegExpRouter = void 0;
const router_1 = require("../../router");
const trie_1 = require("./trie");
const emptyParam = {};
const nullMatcher = [/^$/, []];
function initHint(path) {
    const components = path.match(/\/(?::\w+{[^}]+}|[^\/]*)/g) || [];
    let componentsLength = components.length;
    const paramIndexList = [];
    const regExpComponents = [];
    const namedParams = [];
    for (let i = 0, len = components.length; i < len; i++) {
        if (i === len - 1 && components[i] === '/*') {
            componentsLength--;
            break;
        }
        const m = components[i].match(/^\/:(\w+)({[^}]+})?/);
        if (m) {
            namedParams.push([i, m[1], m[2] || '[^/]+']);
            regExpComponents[i] = m[2] || true;
        }
        else if (components[i] === '/*') {
            regExpComponents[i] = true;
        }
        else {
            regExpComponents[i] = components[i];
        }
        if (/\/(?::|\*)/.test(components[i])) {
            paramIndexList.push(i);
        }
    }
    return {
        components,
        regExpComponents,
        componentsLength,
        endWithWildcard: path.endsWith('*'),
        paramIndexList,
        namedParams,
        maybeHandler: true,
    };
}
function compareRoute(a, b) {
    if (a.path === '*') {
        return 1;
    }
    let i = 0;
    const len = a.hint.regExpComponents.length;
    for (; i < len; i++) {
        if (a.hint.regExpComponents[i] !== b.hint.regExpComponents[i]) {
            if (a.hint.regExpComponents[i] === true) {
                break;
            }
            return 0;
        }
    }
    // may be ambiguous
    for (; i < len; i++) {
        if (a.hint.regExpComponents[i] !== true &&
            a.hint.regExpComponents[i] !== b.hint.regExpComponents[i]) {
            return 2;
        }
    }
    return i === b.hint.regExpComponents.length || a.hint.endWithWildcard ? 1 : 0;
}
function compareHandler(a, b) {
    return a.index - b.index;
}
function getSortedHandlers(handlers) {
    return [...handlers].sort(compareHandler).map((h) => h.handler);
}
function buildMatcherFromPreprocessedRoutes(routes, hasAmbiguous = false) {
    const trie = new trie_1.Trie({ reverse: hasAmbiguous });
    const handlers = [];
    if (routes.length === 0) {
        return nullMatcher;
    }
    for (let i = 0, len = routes.length; i < len; i++) {
        const paramMap = trie.insert(routes[i].path, i);
        handlers[i] = [
            [...routes[i].middleware, ...routes[i].handlers],
            Object.keys(paramMap).length !== 0 ? paramMap : null,
        ];
        if (!hasAmbiguous) {
            handlers[i][0] = getSortedHandlers(handlers[i][0]);
        }
    }
    const [regexp, indexReplacementMap, paramReplacementMap] = trie.buildRegExp();
    for (let i = 0, len = handlers.length; i < len; i++) {
        const paramMap = handlers[i][1];
        if (paramMap) {
            for (let j = 0, len = paramMap.length; j < len; j++) {
                paramMap[j][1] = paramReplacementMap[paramMap[j][1]];
                const aliasTo = routes[i].paramAliasMap[paramMap[j][0]];
                if (aliasTo) {
                    for (let k = 0, len = aliasTo.length; k < len; k++) {
                        paramMap.push([aliasTo[k], paramMap[j][1]]);
                    }
                }
            }
        }
    }
    const handlerMap = [];
    // using `in` because indexReplacementMap is a sparse array
    for (const i in indexReplacementMap) {
        handlerMap[i] = handlers[indexReplacementMap[i]];
    }
    return [regexp, handlerMap];
}
function verifyDuplicateParam(routes) {
    const nameMap = {};
    for (let i = 0, len = routes.length; i < len; i++) {
        const route = routes[i];
        for (let k = 0, len = route.hint.namedParams.length; k < len; k++) {
            const [index, name] = route.hint.namedParams[k];
            if (name in nameMap && index !== nameMap[name]) {
                return false;
            }
            else {
                nameMap[name] = index;
            }
        }
        const paramAliasMap = route.paramAliasMap;
        const paramAliasMapKeys = Object.keys(paramAliasMap);
        for (let k = 0, len = paramAliasMapKeys.length; k < len; k++) {
            const aliasFrom = paramAliasMapKeys[k];
            for (let l = 0, len = paramAliasMap[aliasFrom].length; l < len; l++) {
                const aliasTo = paramAliasMap[aliasFrom][l];
                const index = nameMap[aliasFrom];
                if (aliasTo in nameMap && index !== nameMap[aliasTo]) {
                    return false;
                }
                else {
                    nameMap[aliasTo] = index;
                }
            }
        }
    }
    return true;
}
class RegExpRouter {
    constructor() {
        this.routeData = { index: 0, routes: [], methods: new Set() };
    }
    add(method, path, handler) {
        if (!this.routeData) {
            throw new Error('Can not add a route since the matcher is already built.');
        }
        this.routeData.index++;
        const { index, routes, methods } = this.routeData;
        if (path === '/*') {
            path = '*';
        }
        const hint = initHint(path);
        const handlerWithSortIndex = {
            index,
            handler,
        };
        for (let i = 0, len = routes.length; i < len; i++) {
            if (routes[i].method === method && routes[i].path === path) {
                routes[i].handlers.push(handlerWithSortIndex);
                return;
            }
        }
        methods.add(method);
        routes.push({
            method,
            path,
            hint,
            handlers: [handlerWithSortIndex],
            middleware: [],
            paramAliasMap: {},
        });
    }
    match(method, path) {
        const [primaryMatchers, secondaryMatchers, hasAmbiguous] = this.buildAllMatchers();
        this.match = hasAmbiguous
            ? (method, path) => {
                const matcher = (primaryMatchers[method] ||
                    primaryMatchers[router_1.METHOD_NAME_ALL]);
                let match = path.match(matcher[0]);
                if (!match) {
                    // do not support secondary matchers here.
                    return null;
                }
                const params = {};
                const handlers = new Set();
                let regExpSrc = matcher[0].source;
                while (match) {
                    let index = match.indexOf('', 1);
                    for (;;) {
                        const [handler, paramMap] = matcher[1][index];
                        if (paramMap) {
                            for (let i = 0, len = paramMap.length; i < len; i++) {
                                params[paramMap[i][0]] = match[paramMap[i][1]];
                            }
                        }
                        for (let i = 0, len = handler.length; i < len; i++) {
                            handlers.add(handler[i]);
                        }
                        const newIndex = match.indexOf('', index + 1);
                        if (newIndex === -1) {
                            break;
                        }
                        index = newIndex;
                    }
                    regExpSrc = regExpSrc.replace(new RegExp(`((?:(?:\\(\\?:|.)*?\\([^)]*\\)){${index - 1}}.*?)\\(\\)`), '$1(^)');
                    match = path.match(new RegExp(regExpSrc));
                }
                return { handlers: getSortedHandlers(handlers.values()), params };
            }
            : (method, path) => {
                let matcher = (primaryMatchers[method] || primaryMatchers[router_1.METHOD_NAME_ALL]);
                let match = path.match(matcher[0]);
                if (!match) {
                    const matchers = secondaryMatchers[method] || secondaryMatchers[router_1.METHOD_NAME_ALL];
                    for (let i = 0, len = matchers.length; i < len && !match; i++) {
                        matcher = matchers[i];
                        match = path.match(matcher[0]);
                    }
                    if (!match) {
                        return null;
                    }
                }
                const index = match.indexOf('', 1);
                const [handlers, paramMap] = matcher[1][index];
                if (!paramMap) {
                    return { handlers, params: emptyParam };
                }
                const params = {};
                for (let i = 0, len = paramMap.length; i < len; i++) {
                    params[paramMap[i][0]] = match[paramMap[i][1]];
                }
                return { handlers, params };
            };
        return this.match(method, path);
    }
    buildAllMatchers() {
        // @ts-ignore
        this.routeData.routes.sort(({ hint: a }, { hint: b }) => {
            if (a.componentsLength !== b.componentsLength) {
                return a.componentsLength - b.componentsLength;
            }
            for (let i = 0, len = Math.min(a.paramIndexList.length, b.paramIndexList.length) + 1; i < len; i++) {
                if (a.paramIndexList[i] !== b.paramIndexList[i]) {
                    if (a.paramIndexList[i] === undefined) {
                        return -1;
                    }
                    else if (b.paramIndexList[i] === undefined) {
                        return 1;
                    }
                    else {
                        return a.paramIndexList[i] - b.paramIndexList[i];
                    }
                }
            }
            if (a.endWithWildcard !== b.endWithWildcard) {
                return a.endWithWildcard ? -1 : 1;
            }
            return 0;
        });
        const primaryMatchers = {};
        const secondaryMatchers = {};
        let hasAmbiguous = false;
        // @ts-ignore
        this.routeData.methods.forEach((method) => {
            let _hasAmbiguous;
            [primaryMatchers[method], secondaryMatchers[method], _hasAmbiguous] =
                this.buildMatcher(method);
            hasAmbiguous = hasAmbiguous || _hasAmbiguous;
        });
        primaryMatchers[router_1.METHOD_NAME_ALL] || (primaryMatchers[router_1.METHOD_NAME_ALL] = nullMatcher);
        secondaryMatchers[router_1.METHOD_NAME_ALL] || (secondaryMatchers[router_1.METHOD_NAME_ALL] = []);
        delete this.routeData; // to reduce memory usage
        return [primaryMatchers, secondaryMatchers, hasAmbiguous];
    }
    buildMatcher(method) {
        var _a, _b;
        let hasAmbiguous = false;
        const targetMethods = new Set([method, router_1.METHOD_NAME_ALL]);
        // @ts-ignore
        const routes = this.routeData.routes.filter(({ method }) => targetMethods.has(method));
        // Reset temporary data per method
        for (let i = 0, len = routes.length; i < len; i++) {
            routes[i].middleware = [];
            routes[i].paramAliasMap = {};
        }
        // preprocess routes
        for (let i = 0, len = routes.length; i < len; i++) {
            for (let j = i + 1; j < len; j++) {
                const compareResult = compareRoute(routes[i], routes[j]);
                // i includes j
                if (compareResult === 1) {
                    const components = routes[j].hint.components;
                    const namedParams = routes[i].hint.namedParams;
                    for (let k = 0, len = namedParams.length; k < len; k++) {
                        const c = components[namedParams[k][0]];
                        const m = c.match(/^\/:(\w+)({[^}]+})?/);
                        if (m && namedParams[k][1] === m[1]) {
                            continue;
                        }
                        if (m) {
                            (_a = routes[j].paramAliasMap)[_b = m[1]] || (_a[_b] = []);
                            routes[j].paramAliasMap[m[1]].push(namedParams[k][1]);
                        }
                        else {
                            components[namedParams[k][0]] = `/:${namedParams[k][1]}{${c.substring(1)}}`;
                            routes[j].hint.namedParams.push([
                                namedParams[k][0],
                                namedParams[k][1],
                                c.substring(1),
                            ]);
                            routes[j].path = components.join('');
                        }
                    }
                    if (routes[j].hint.components.length < routes[i].hint.components.length) {
                        routes[j].middleware.push(...routes[i].handlers.map((h) => ({
                            index: h.index,
                            handler: h.handler,
                        })));
                    }
                    else {
                        routes[j].middleware.push(...routes[i].handlers);
                    }
                    routes[i].hint.maybeHandler = false;
                }
                else if (compareResult === 2) {
                    // ambiguous
                    hasAmbiguous = true;
                    if (!verifyDuplicateParam([routes[i], routes[j]])) {
                        throw new Error('Duplicate param name');
                    }
                }
            }
            if (!verifyDuplicateParam([routes[i]])) {
                throw new Error('Duplicate param name');
            }
        }
        if (hasAmbiguous) {
            return [buildMatcherFromPreprocessedRoutes(routes, hasAmbiguous), [], hasAmbiguous];
        }
        const primaryRoutes = [];
        const secondaryRoutes = [];
        for (let i = 0, len = routes.length; i < len; i++) {
            if (routes[i].hint.maybeHandler || !routes[i].hint.endWithWildcard) {
                primaryRoutes.push(routes[i]);
            }
            else {
                secondaryRoutes.push(routes[i]);
            }
        }
        return [
            buildMatcherFromPreprocessedRoutes(primaryRoutes, hasAmbiguous),
            [buildMatcherFromPreprocessedRoutes(secondaryRoutes, hasAmbiguous)],
            hasAmbiguous,
        ];
    }
}
exports.RegExpRouter = RegExpRouter;
