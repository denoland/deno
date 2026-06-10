"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Trie = void 0;
const node_1 = require("./node");
class Trie {
    constructor({ reverse } = { reverse: false }) {
        this.context = { varIndex: 0 };
        this.root = new node_1.Node({ reverse });
    }
    insert(path, index) {
        const paramMap = [];
        /**
         *  - pattern (:label, :label{0-9]+}, ...)
         *  - /* wildcard
         *  - character
         */
        const tokens = path.match(/(?::[^\/]+)|(?:\/\*$)|./g);
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
        this.root.insert(tokens, index, paramMap, this.context);
        return paramMap;
    }
    buildRegExp() {
        let regexp = this.root.buildRegExpStr();
        let captureIndex = 0;
        const indexReplacementMap = [];
        const paramReplacementMap = [];
        regexp = regexp.replace(/#(\d+)|@(\d+)|\.\*\$/g, (_, handlerIndex, paramIndex) => {
            if (typeof handlerIndex !== 'undefined') {
                indexReplacementMap[++captureIndex] = Number(handlerIndex);
                return '$()';
            }
            if (typeof paramIndex !== 'undefined') {
                paramReplacementMap[Number(paramIndex)] = ++captureIndex;
                return '';
            }
            return '';
        });
        return [new RegExp(`^${regexp}`), indexReplacementMap, paramReplacementMap];
    }
}
exports.Trie = Trie;
