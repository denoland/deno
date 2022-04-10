var register_1 = register;
function register(state, name, method, options) {
  if (typeof method !== "function") {
    throw new Error("method for before hook must be a function");
  }
  if (!options) {
    options = {};
  }
  if (Array.isArray(name)) {
    return name.reverse().reduce(function(callback, name2) {
      return register.bind(null, state, name2, callback, options);
    }, method)();
  }
  return Promise.resolve().then(function() {
    if (!state.registry[name]) {
      return method(options);
    }
    return state.registry[name].reduce(function(method2, registered) {
      return registered.hook.bind(null, method2, options);
    }, method)();
  });
}
var add = addHook;
function addHook(state, kind, name, hook) {
  var orig = hook;
  if (!state.registry[name]) {
    state.registry[name] = [];
  }
  if (kind === "before") {
    hook = function(method, options) {
      return Promise.resolve().then(orig.bind(null, options)).then(method.bind(null, options));
    };
  }
  if (kind === "after") {
    hook = function(method, options) {
      var result;
      return Promise.resolve().then(method.bind(null, options)).then(function(result_) {
        result = result_;
        return orig(result, options);
      }).then(function() {
        return result;
      });
    };
  }
  if (kind === "error") {
    hook = function(method, options) {
      return Promise.resolve().then(method.bind(null, options)).catch(function(error) {
        return orig(error, options);
      });
    };
  }
  state.registry[name].push({
    hook,
    orig
  });
}
var remove = removeHook;
function removeHook(state, name, method) {
  if (!state.registry[name]) {
    return;
  }
  var index = state.registry[name].map(function(registered) {
    return registered.orig;
  }).indexOf(method);
  if (index === -1) {
    return;
  }
  state.registry[name].splice(index, 1);
}
var bind = Function.bind;
var bindable = bind.bind(bind);
function bindApi(hook, state, name) {
  var removeHookRef = bindable(remove, null).apply(null, name ? [state, name] : [state]);
  hook.api = {remove: removeHookRef};
  hook.remove = removeHookRef;
  ["before", "error", "after", "wrap"].forEach(function(kind) {
    var args = name ? [state, kind, name] : [state, kind];
    hook[kind] = hook.api[kind] = bindable(add, null).apply(null, args);
  });
}
function HookSingular() {
  var singularHookName = "h";
  var singularHookState = {
    registry: {}
  };
  var singularHook = register_1.bind(null, singularHookState, singularHookName);
  bindApi(singularHook, singularHookState, singularHookName);
  return singularHook;
}
function HookCollection() {
  var state = {
    registry: {}
  };
  var hook = register_1.bind(null, state);
  bindApi(hook, state);
  return hook;
}
var collectionHookDeprecationMessageDisplayed = false;
function Hook() {
  if (!collectionHookDeprecationMessageDisplayed) {
    console.warn('[before-after-hook]: "Hook()" repurposing warning, use "Hook.Collection()". Read more: https://git.io/upgrade-before-after-hook-to-1.4');
    collectionHookDeprecationMessageDisplayed = true;
  }
  return HookCollection();
}
Hook.Singular = HookSingular.bind();
Hook.Collection = HookCollection.bind();
var beforeAfterHook = Hook;
var Hook_1 = Hook;
var Singular = Hook.Singular;
var Collection = Hook.Collection;
beforeAfterHook.Hook = Hook_1;
beforeAfterHook.Singular = Singular;
beforeAfterHook.Collection = Collection;
export default beforeAfterHook;
export {Collection, Hook_1 as Hook, Singular, beforeAfterHook as __moduleExports};
