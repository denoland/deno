import { getUserAgent } from "/-/universal-user-agent@v6.0.0-fUAPE3UH5QP7qG0fd0dH/dist=es2019,mode=imports/optimized/universal-user-agent.js";
import __commonjs_module0 from "/-/before-after-hook@v2.2.2-pi5OVaqfPuA5i8u2q0Od/dist=es2019,mode=imports/optimized/before-after-hook.js";
const { Collection } = __commonjs_module0;

import { request as request2 } from "/-/@octokit/request@v5.6.3-8MiyZSoy8B73C1K9nYC8/dist=es2019,mode=imports/optimized/@octokit/request.js";
import { withCustomRequest } from "/-/@octokit/graphql@v4.8.0-EbMgAhtBEVS5THey9fbY/dist=es2019,mode=imports/optimized/@octokit/graphql.js";
import { createTokenAuth } from "/-/@octokit/auth-token@v2.5.0-63e6RmuUEnR1eflFvHL4/dist=es2019,mode=imports/optimized/@octokit/auth-token.js";
const VERSION = "3.6.0";
class Octokit {
  constructor(options = {}) {
    const hook = new Collection();
    const requestDefaults = {
      baseUrl: request2.endpoint.DEFAULTS.baseUrl,
      headers: {},
      request: Object.assign({}, options.request, {
        hook: hook.bind(null, "request"),
      }),
      mediaType: {
        previews: [],
        format: "",
      },
    };
    requestDefaults.headers["user-agent"] = [
      options.userAgent,
      `octokit-core.js/${VERSION} ${getUserAgent()}`,
    ].filter(Boolean).join(" ");
    if (options.baseUrl) {
      requestDefaults.baseUrl = options.baseUrl;
    }
    if (options.previews) {
      requestDefaults.mediaType.previews = options.previews;
    }
    if (options.timeZone) {
      requestDefaults.headers["time-zone"] = options.timeZone;
    }
    this.request = request2.defaults(requestDefaults);
    this.graphql = withCustomRequest(this.request).defaults(requestDefaults);
    this.log = Object.assign({
      debug: () => {
      },
      info: () => {
      },
      warn: console.warn.bind(console),
      error: console.error.bind(console),
    }, options.log);
    this.hook = hook;
    if (!options.authStrategy) {
      if (!options.auth) {
        this.auth = async () => ({
          type: "unauthenticated",
        });
      } else {
        const auth = createTokenAuth(options.auth);
        hook.wrap("request", auth.hook);
        this.auth = auth;
      }
    } else {
      const { authStrategy, ...otherOptions } = options;
      const auth = authStrategy(Object.assign({
        request: this.request,
        log: this.log,
        octokit: this,
        octokitOptions: otherOptions,
      }, options.auth));
      hook.wrap("request", auth.hook);
      this.auth = auth;
    }
    const classConstructor = this.constructor;
    classConstructor.plugins.forEach((plugin) => {
      Object.assign(this, plugin(this, options));
    });
  }
  static defaults(defaults) {
    const OctokitWithDefaults = class extends this {
      constructor(...args) {
        const options = args[0] || {};
        if (typeof defaults === "function") {
          super(defaults(options));
          return;
        }
        super(
          Object.assign(
            {},
            defaults,
            options,
            options.userAgent && defaults.userAgent
              ? {
                userAgent: `${options.userAgent} ${defaults.userAgent}`,
              }
              : null,
          ),
        );
      }
    };
    return OctokitWithDefaults;
  }
  static plugin(...newPlugins) {
    var _a;
    const currentPlugins = this.plugins;
    const NewOctokit = (_a = class extends this {
    },
      _a.plugins = currentPlugins.concat(
        newPlugins.filter((plugin) => !currentPlugins.includes(plugin)),
      ),
      _a);
    return NewOctokit;
  }
}
Octokit.VERSION = VERSION;
Octokit.plugins = [];
export { Octokit };
export default null;
