"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.TrieRouter = void 0;
const node_1 = require("./node");
class TrieRouter {
    constructor() {
        this.node = new node_1.Node();
    }
    add(method, path, handler) {
        this.node.insert(method, path, handler);
    }
    match(method, path) {
        return this.node.search(method, path);
    }
}
exports.TrieRouter = TrieRouter;
