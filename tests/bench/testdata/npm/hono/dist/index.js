"use strict";
// @denoify-ignore
// eslint-disable-next-line @typescript-eslint/triple-slash-reference
/// <reference path="./request.ts" /> Import "declare global" for the Request interface.
Object.defineProperty(exports, "__esModule", { value: true });
exports.Hono = void 0;
const hono_1 = require("./hono");
Object.defineProperty(exports, "Hono", { enumerable: true, get: function () { return hono_1.Hono; } });
hono_1.Hono.prototype.fire = function () {
    addEventListener('fetch', (event) => {
        void event.respondWith(this.handleEvent(event));
    });
};
