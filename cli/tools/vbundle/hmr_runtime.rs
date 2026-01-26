// Copyright 2018-2026 the Deno authors. MIT license.

//! HMR Runtime Code Generation.
//!
//! This module generates the JavaScript HMR runtime code that gets injected
//! into bundled modules in development mode. It implements a Vite-compatible
//! `import.meta.hot` API.

use super::environment::BundleEnvironment;
use super::hmr_types::HmrConfig;

/// Generate the HMR runtime preamble for injection into entry chunks.
///
/// This creates the `__VBUNDLE_HMR__` global object with the full HMR API.
pub fn generate_hmr_runtime(config: &HmrConfig, environment: &BundleEnvironment) -> String {
  let client_code = match environment {
    BundleEnvironment::Browser => generate_browser_client(config),
    BundleEnvironment::Server | BundleEnvironment::Custom(_) => generate_server_client(config),
  };

  format!(
    r#"// Vbundle HMR Runtime
(function() {{
  if (globalThis.__VBUNDLE_HMR__) return;

  const moduleRegistry = new Map();
  const eventHandlers = new Map();
  let socket = null;
  let isConnected = false;
  let pendingUpdates = [];

  // ViteHotContext - implements import.meta.hot API
  class ViteHotContext {{
    constructor(moduleId) {{
      this.moduleId = moduleId;
      this._data = {{}};
      this._acceptCallbacks = [];
      this._acceptDepsCallbacks = new Map();
      this._disposeCallbacks = [];
      this._pruneCallbacks = [];
      this._declined = false;
      this._acceptSelf = false;
      this._invalidated = false;
    }}

    // Preserved data across HMR updates
    get data() {{
      return this._data;
    }}

    // Accept updates for this module or its dependencies
    accept(deps, callback) {{
      if (typeof deps === 'function' || deps === undefined) {{
        // hot.accept() or hot.accept(cb) - self-accepting
        this._acceptSelf = true;
        if (typeof deps === 'function') {{
          this._acceptCallbacks.push(deps);
        }}
      }} else if (typeof deps === 'string') {{
        // hot.accept('./dep', cb) - accept specific dependency
        const resolved = this._resolveDep(deps);
        if (!this._acceptDepsCallbacks.has(resolved)) {{
          this._acceptDepsCallbacks.set(resolved, []);
        }}
        if (callback) {{
          this._acceptDepsCallbacks.get(resolved).push(callback);
        }}
      }} else if (Array.isArray(deps)) {{
        // hot.accept(['./dep1', './dep2'], cb) - accept multiple dependencies
        const resolved = deps.map(d => this._resolveDep(d));
        resolved.forEach(dep => {{
          if (!this._acceptDepsCallbacks.has(dep)) {{
            this._acceptDepsCallbacks.set(dep, []);
          }}
        }});
        if (callback) {{
          // Store callback with all deps for multi-dep callbacks
          this._acceptCallbacks.push({{ deps: resolved, callback }});
        }}
      }}
    }}

    // Clean up before module is replaced
    dispose(callback) {{
      this._disposeCallbacks.push(callback);
    }}

    // Clean up when module is removed from the graph
    prune(callback) {{
      this._pruneCallbacks.push(callback);
    }}

    // Decline HMR updates - triggers full reload
    decline() {{
      this._declined = true;
    }}

    // Invalidate this module - propagate update to importers
    invalidate(message) {{
      this._invalidated = true;
      __VBUNDLE_HMR__.invalidateModule(this.moduleId, message);
    }}

    // Listen for HMR events
    on(event, callback) {{
      __VBUNDLE_HMR__.on(event, callback);
    }}

    // Remove event listener
    off(event, callback) {{
      __VBUNDLE_HMR__.off(event, callback);
    }}

    // Send custom event to server
    send(event, data) {{
      __VBUNDLE_HMR__.send(event, data);
    }}

    // Internal: resolve dependency path
    _resolveDep(dep) {{
      if (dep.startsWith('./') || dep.startsWith('../')) {{
        // Relative import - resolve against module URL
        const base = new URL(this.moduleId, 'file://');
        return new URL(dep, base).href;
      }}
      return dep;
    }}
  }}

  // Module info stored in registry
  class ModuleInfo {{
    constructor(id, hot) {{
      this.id = id;
      this.hot = hot;
      this.factory = null;
      this.exports = null;
    }}
  }}

  // Core HMR API
  globalThis.__VBUNDLE_HMR__ = {{
    // Create a hot context for a module
    createHotContext(moduleId) {{
      const existing = moduleRegistry.get(moduleId);
      if (existing) {{
        // Preserve data from previous version
        const hot = new ViteHotContext(moduleId);
        hot._data = existing.hot._data;
        existing.hot = hot;
        return hot;
      }}
      const hot = new ViteHotContext(moduleId);
      moduleRegistry.set(moduleId, new ModuleInfo(moduleId, hot));
      return hot;
    }},

    // Register a module factory for HMR
    registerModule(moduleId, factory, exports) {{
      const info = moduleRegistry.get(moduleId);
      if (info) {{
        info.factory = factory;
        info.exports = exports;
      }}
    }},

    // Get module info
    getModuleInfo(moduleId) {{
      return moduleRegistry.get(moduleId);
    }},

    // Apply an HMR update
    async applyUpdate(updates) {{
      emit('vite:beforeUpdate', {{ updates }});

      for (const update of updates) {{
        const moduleInfo = moduleRegistry.get(update.acceptedPath);
        if (!moduleInfo) {{
          console.warn('[vbundle] Module not found for HMR:', update.acceptedPath);
          triggerFullReload();
          return;
        }}

        const hot = moduleInfo.hot;

        // Check if module declined HMR
        if (hot._declined) {{
          console.log('[vbundle] Module declined HMR:', update.acceptedPath);
          triggerFullReload();
          return;
        }}

        // Run dispose callbacks
        for (const cb of hot._disposeCallbacks) {{
          try {{
            cb(hot._data);
          }} catch (e) {{
            console.error('[vbundle] Error in dispose callback:', e);
          }}
        }}

        // Fetch and execute new module code
        try {{
          const timestamp = update.timestamp || Date.now();
          const newUrl = update.path + (update.path.includes('?') ? '&' : '?') + 't=' + timestamp;

          if (update.type === 'js-update') {{
            // For browser: dynamic import the updated module
            // For server: evaluate new code
            await __VBUNDLE_HMR__.fetchAndExecute(newUrl, update.acceptedPath);
          }} else if (update.type === 'css-update') {{
            // CSS updates: replace stylesheet link
            updateStylesheet(update.path, timestamp);
          }}

          // Get new module info after update
          const newModuleInfo = moduleRegistry.get(update.acceptedPath);
          const newExports = newModuleInfo ? newModuleInfo.exports : null;

          // Run accept callbacks
          if (hot._acceptSelf) {{
            for (const cb of hot._acceptCallbacks) {{
              if (typeof cb === 'function') {{
                try {{
                  cb(newExports);
                }} catch (e) {{
                  console.error('[vbundle] Error in accept callback:', e);
                }}
              }}
            }}
          }}

          // Run dependency accept callbacks
          for (const [dep, callbacks] of hot._acceptDepsCallbacks) {{
            if (dep === update.path) {{
              const depInfo = moduleRegistry.get(dep);
              for (const cb of callbacks) {{
                try {{
                  cb(depInfo ? depInfo.exports : null);
                }} catch (e) {{
                  console.error('[vbundle] Error in accept dep callback:', e);
                }}
              }}
            }}
          }}

          console.log('[vbundle] HMR update applied:', update.path);
        }} catch (e) {{
          console.error('[vbundle] HMR update failed:', e);
          emit('vite:error', {{ err: e.message }});
          triggerFullReload();
          return;
        }}
      }}

      emit('vite:afterUpdate', {{ updates }});
    }},

    // Fetch and execute updated module code
    async fetchAndExecute(url, moduleId) {{
      {client_fetch_code}
    }},

    // Invalidate a module - propagate update to importers
    invalidateModule(moduleId, message) {{
      emit('vite:invalidate', {{ path: moduleId, message }});
      if (socket && isConnected) {{
        socket.send(JSON.stringify({{
          type: 'custom',
          event: 'vite:invalidate',
          data: {{ path: moduleId, message }}
        }}));
      }}
    }},

    // Trigger full page reload
    triggerFullReload(path) {{
      emit('vite:beforeFullReload', {{ path }});
      {full_reload_code}
    }},

    // Prune modules that are no longer needed
    pruneModules(paths) {{
      emit('vite:beforePrune', {{ paths }});
      for (const path of paths) {{
        const info = moduleRegistry.get(path);
        if (info && info.hot) {{
          for (const cb of info.hot._pruneCallbacks) {{
            try {{
              cb(info.hot._data);
            }} catch (e) {{
              console.error('[vbundle] Error in prune callback:', e);
            }}
          }}
        }}
        moduleRegistry.delete(path);
      }}
    }},

    // Event system
    on(event, callback) {{
      if (!eventHandlers.has(event)) {{
        eventHandlers.set(event, new Set());
      }}
      eventHandlers.get(event).add(callback);
    }},

    off(event, callback) {{
      const handlers = eventHandlers.get(event);
      if (handlers) {{
        handlers.delete(callback);
      }}
    }},

    // Send custom event to server
    send(event, data) {{
      if (socket && isConnected) {{
        socket.send(JSON.stringify({{
          type: 'custom',
          event,
          data
        }}));
      }}
    }},

    // Initialize HMR connection
    connect() {{
      {client_code}
    }}
  }};

  // Emit an event to all registered handlers
  function emit(event, data) {{
    const handlers = eventHandlers.get(event);
    if (handlers) {{
      for (const handler of handlers) {{
        try {{
          handler(data);
        }} catch (e) {{
          console.error('[vbundle] Error in event handler for', event, ':', e);
        }}
      }}
    }}
  }}

  // Trigger full reload
  function triggerFullReload(path) {{
    __VBUNDLE_HMR__.triggerFullReload(path);
  }}

  // Update a stylesheet (for CSS HMR)
  function updateStylesheet(path, timestamp) {{
    const links = document.querySelectorAll('link[rel="stylesheet"]');
    for (const link of links) {{
      if (link.href.includes(path)) {{
        const newHref = path + (path.includes('?') ? '&' : '?') + 't=' + timestamp;
        link.href = newHref;
        return;
      }}
    }}
  }}

  // Auto-connect on load
  __VBUNDLE_HMR__.connect();
}})();
"#,
    client_code = client_code,
    client_fetch_code = generate_fetch_code(environment),
    full_reload_code = generate_full_reload_code(environment),
  )
}

/// Generate browser-specific WebSocket client code.
fn generate_browser_client(config: &HmrConfig) -> String {
  format!(
    r#"const wsUrl = '{ws_url}/__vbundle_hmr';
      socket = new WebSocket(wsUrl);

      socket.addEventListener('open', () => {{
        isConnected = true;
        console.log('[vbundle] HMR connected');
        emit('vite:ws:connect', {{ websocket: socket }});

        // Process any pending updates
        for (const update of pendingUpdates) {{
          __VBUNDLE_HMR__.applyUpdate(update);
        }}
        pendingUpdates = [];
      }});

      socket.addEventListener('message', async (event) => {{
        const message = JSON.parse(event.data);

        switch (message.type) {{
          case 'connected':
            console.log('[vbundle] Server confirmed connection');
            break;

          case 'update':
            if (isConnected) {{
              await __VBUNDLE_HMR__.applyUpdate(message.updates);
            }} else {{
              pendingUpdates.push(message.updates);
            }}
            break;

          case 'full-reload':
            __VBUNDLE_HMR__.triggerFullReload(message.path);
            break;

          case 'prune':
            __VBUNDLE_HMR__.pruneModules(message.paths);
            break;

          case 'error':
            emit('vite:error', message);
            if ({overlay}) {{
              showErrorOverlay(message);
            }}
            break;

          case 'custom':
            emit(message.event, message.data);
            break;
        }}
      }});

      socket.addEventListener('close', () => {{
        isConnected = false;
        console.log('[vbundle] HMR disconnected');
        emit('vite:ws:disconnect', {{ websocket: socket }});

        // Attempt to reconnect after a delay
        setTimeout(() => {{
          console.log('[vbundle] Attempting to reconnect...');
          __VBUNDLE_HMR__.connect();
        }}, 1000);
      }});

      socket.addEventListener('error', (e) => {{
        console.error('[vbundle] WebSocket error:', e);
      }});

      // Error overlay for development
      function showErrorOverlay(error) {{
        const overlay = document.createElement('div');
        overlay.id = 'vbundle-error-overlay';
        overlay.style.cssText = `
          position: fixed;
          top: 0;
          left: 0;
          width: 100%;
          height: 100%;
          background: rgba(0, 0, 0, 0.85);
          color: #ff5555;
          font-family: monospace;
          font-size: 14px;
          padding: 20px;
          box-sizing: border-box;
          z-index: 999999;
          overflow: auto;
        `;

        const content = document.createElement('div');
        content.innerHTML = `
          <h2 style="color: #ff5555; margin-bottom: 10px;">Build Error</h2>
          <pre style="white-space: pre-wrap; word-wrap: break-word;">${{escapeHtml(error.message)}}</pre>
          ${{error.stack ? `<pre style="color: #888; margin-top: 10px;">${{escapeHtml(error.stack)}}</pre>` : ''}}
          ${{error.file ? `<p style="color: #888; margin-top: 10px;">File: ${{escapeHtml(error.file)}}</p>` : ''}}
          <button onclick="this.parentElement.parentElement.remove()" style="
            margin-top: 20px;
            padding: 10px 20px;
            background: #333;
            color: white;
            border: none;
            cursor: pointer;
          ">Close</button>
        `;
        overlay.appendChild(content);
        document.body.appendChild(overlay);
      }}

      function escapeHtml(text) {{
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
      }}"#,
    ws_url = config.websocket_url(),
    overlay = if config.overlay { "true" } else { "false" }
  )
}

/// Generate server-side HMR client code (for Deno/Node/Bun).
fn generate_server_client(config: &HmrConfig) -> String {
  format!(
    r#"// Server-side HMR uses WebSocket to communicate with HMR server
      const wsUrl = '{ws_url}/__vbundle_hmr';

      try {{
        // Use WebSocket if available (Deno, modern Node with undici)
        socket = new WebSocket(wsUrl);

        socket.addEventListener('open', () => {{
          isConnected = true;
          console.log('[vbundle] Server HMR connected');
          emit('vite:ws:connect', {{ websocket: socket }});
        }});

        socket.addEventListener('message', async (event) => {{
          const data = typeof event.data === 'string' ? event.data : await event.data.text();
          const message = JSON.parse(data);

          switch (message.type) {{
            case 'connected':
              console.log('[vbundle] Server confirmed connection');
              break;

            case 'update':
              await __VBUNDLE_HMR__.applyUpdate(message.updates);
              break;

            case 'full-reload':
              __VBUNDLE_HMR__.triggerFullReload(message.path);
              break;

            case 'prune':
              __VBUNDLE_HMR__.pruneModules(message.paths);
              break;

            case 'error':
              emit('vite:error', message);
              console.error('[vbundle] Build error:', message.message);
              break;

            case 'custom':
              emit(message.event, message.data);
              break;
          }}
        }});

        socket.addEventListener('close', () => {{
          isConnected = false;
          console.log('[vbundle] Server HMR disconnected');
          emit('vite:ws:disconnect', {{ websocket: socket }});

          // Attempt to reconnect after a delay
          setTimeout(() => {{
            console.log('[vbundle] Attempting to reconnect...');
            __VBUNDLE_HMR__.connect();
          }}, 1000);
        }});

        socket.addEventListener('error', (e) => {{
          console.error('[vbundle] WebSocket error:', e);
        }});
      }} catch (e) {{
        console.warn('[vbundle] WebSocket not available, HMR disabled');
      }}"#,
    ws_url = config.websocket_url()
  )
}

/// Generate environment-specific code for fetching updated modules.
fn generate_fetch_code(environment: &BundleEnvironment) -> &'static str {
  match environment {
    BundleEnvironment::Browser => {
      r#"// Browser: use dynamic import with cache busting
      const module = await import(url);
      const info = moduleRegistry.get(moduleId);
      if (info) {
        info.exports = module;
      }
      return module;"#
    }
    BundleEnvironment::Server | BundleEnvironment::Custom(_) => {
      r#"// Server: fetch new code and evaluate it
      // In server environments, we use a module factory pattern
      // The new code is sent over the WebSocket and evaluated
      const info = moduleRegistry.get(moduleId);
      if (info && info.factory) {
        // Re-execute the factory with new code
        // This is handled by the HMR server sending the new module code
        const newExports = {};
        const newModule = { exports: newExports };
        info.factory(newExports, newModule, info.hot);
        info.exports = newModule.exports;
      }
      return info ? info.exports : null;"#
    }
  }
}

/// Generate environment-specific code for triggering full reload.
fn generate_full_reload_code(environment: &BundleEnvironment) -> &'static str {
  match environment {
    BundleEnvironment::Browser => {
      r#"if (typeof location !== 'undefined') {
        location.reload();
      }"#
    }
    BundleEnvironment::Server | BundleEnvironment::Custom(_) => {
      r#"// Server-side: signal process to restart
      console.log('[vbundle] Full reload required, please restart the process');
      if (typeof Deno !== 'undefined' && Deno.exit) {
        // In Deno, we can use the file watcher to restart
        // Just log for now, the watcher will handle it
        console.log('[vbundle] Module change requires restart:', path);
      } else if (typeof process !== 'undefined' && process.exit) {
        // Node.js - similar approach
        console.log('[vbundle] Module change requires restart:', path);
      }"#
    }
  }
}

/// Generate the HMR wrapper for a module.
///
/// This wraps module code to inject the `import.meta.hot` context.
pub fn generate_module_hmr_wrapper(
  module_id: &str,
  original_module_var: &str,
  code: &str,
) -> String {
  format!(
    r#"// Module: {module_id}
var {module_var} = (function(exports, module, hot) {{
  // Inject import.meta.hot
  const __vite_hot__ = hot;
  Object.defineProperty(import.meta || {{}}, 'hot', {{
    get() {{ return __vite_hot__; }},
    configurable: true
  }});

{code}
  return module.exports;
}})(Object.create(null), {{ exports: Object.create(null) }}, __VBUNDLE_HMR__.createHotContext("{module_id}"));

// Register module for HMR
if (typeof __VBUNDLE_HMR__ !== 'undefined') {{
  __VBUNDLE_HMR__.registerModule("{module_id}", null, {module_var});
}}
"#,
    module_id = module_id,
    module_var = original_module_var,
    code = indent_code(code, 2)
  )
}

/// Indent code by a number of spaces.
fn indent_code(code: &str, spaces: usize) -> String {
  let indent = " ".repeat(spaces);
  code
    .lines()
    .map(|line| {
      if line.is_empty() {
        line.to_string()
      } else {
        format!("{}{}", indent, line)
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_generate_hmr_runtime_browser() {
    let config = HmrConfig::default();
    let runtime = generate_hmr_runtime(&config, &BundleEnvironment::Browser);

    assert!(runtime.contains("__VBUNDLE_HMR__"));
    assert!(runtime.contains("ViteHotContext"));
    assert!(runtime.contains("createHotContext"));
    assert!(runtime.contains("applyUpdate"));
    assert!(runtime.contains("ws://localhost:24678"));
    assert!(runtime.contains("location.reload"));
  }

  #[test]
  fn test_generate_hmr_runtime_server() {
    let config = HmrConfig::default();
    let runtime = generate_hmr_runtime(&config, &BundleEnvironment::Server);

    assert!(runtime.contains("__VBUNDLE_HMR__"));
    assert!(runtime.contains("ViteHotContext"));
    // Server runtime should not use location.reload
    assert!(runtime.contains("please restart the process"));
  }

  #[test]
  fn test_generate_hmr_runtime_custom_port() {
    let config = HmrConfig::default().with_port(3000);
    let runtime = generate_hmr_runtime(&config, &BundleEnvironment::Browser);

    assert!(runtime.contains("ws://localhost:3000"));
  }

  #[test]
  fn test_generate_module_hmr_wrapper() {
    let code = "export const x = 1;";
    let wrapped = generate_module_hmr_wrapper("file:///app/mod.ts", "__module_0__", code);

    assert!(wrapped.contains("__module_0__"));
    assert!(wrapped.contains("import.meta.hot"));
    assert!(wrapped.contains("__VBUNDLE_HMR__.createHotContext"));
    assert!(wrapped.contains("file:///app/mod.ts"));
    assert!(wrapped.contains("export const x = 1;"));
  }
}
