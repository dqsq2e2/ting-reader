//! JavaScript init code for plugin runtime
//!
//! Generates the initialization JavaScript that sets up the Ting API globals
//! (Ting, fetch, Headers/URL polyfills, _ting_invoke) in the Deno runtime.

use serde_json::Value;

/// Generate the JavaScript init code that bootstraps the Ting environment
pub fn generate_init_code(
    plugin_name: &str,
    config: &Value,
    allowed_paths: &[String],
    allowed_domains: &[String],
) -> String {
    let config_json = serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string());
    let paths_json = serde_json::to_string(allowed_paths).unwrap_or_else(|_| "[]".to_string());
    let domains_json = serde_json::to_string(allowed_domains).unwrap_or_else(|_| "[]".to_string());

    format!(r#"
        "use strict";

        // Polyfill Headers
        globalThis.Headers = class Headers {{
            constructor(init) {{
                this.map = new Map();
                if (init) {{
                    if (init instanceof Headers) {{
                        init.forEach((value, key) => this.append(key, value));
                    }} else if (Array.isArray(init)) {{
                        init.forEach(([key, value]) => this.append(key, value));
                    }} else {{
                        Object.keys(init).forEach(key => this.append(key, init[key]));
                    }}
                }}
            }}
            append(name, value) {{
                name = name.toLowerCase();
                value = String(value);
                if (this.map.has(name)) {{
                    this.map.get(name).push(value);
                }} else {{
                    this.map.set(name, [value]);
                }}
            }}
            delete(name) {{ this.map.delete(name.toLowerCase()); }}
            get(name) {{
                const values = this.map.get(name.toLowerCase());
                return values ? values[0] : null;
            }}
            has(name) {{ return this.map.has(name.toLowerCase()); }}
            set(name, value) {{ this.map.set(name.toLowerCase(), [String(value)]); }}
            forEach(callback, thisArg) {{
                for (const [name, values] of this.map) {{
                    callback.call(thisArg, values.join(', '), name, this);
                }}
            }}
        }};

        // Polyfill URL (Minimal)
        globalThis.URL = class URL {{
            constructor(url, base) {{
                if (base) {{
                    if (base.endsWith('/')) base = base.slice(0, -1);
                    if (!url.startsWith('/')) url = '/' + url;
                    url = base + url;
                }}
                this.href = url;
                const match = url.match(/^(https?:)\/\/([^/?#]+)(.*)$/);
                if (match) {{
                    this.protocol = match[1];
                    this.hostname = match[2];
                    this.pathname = match[3] || '/';
                    this.search = '';
                    if (this.pathname.includes('?')) {{
                        const parts = this.pathname.split('?');
                        this.pathname = parts[0];
                        this.search = '?' + parts[1];
                    }}
                }} else {{
                    this.hostname = '';
                    this.protocol = '';
                    this.pathname = '';
                    this.search = '';
                }}
            }}
            toString() {{ return this.href; }}
        }};

        // Ting Plugin API for JavaScript
        globalThis.Ting = {{
            pluginName: "{plugin_name}",
            config: {config_json},

            // Sandbox information
            sandbox: {{
                allowedPaths: {paths_json},
                allowedDomains: {domains_json},
            }},

            // Logging functions
            log: {{
                debug: (message) => console.log(`[DEBUG] [{plugin_name}] ${{message}}`),
                info: (message) => console.log(`[INFO] [{plugin_name}] ${{message}}`),
                warn: (message) => console.warn(`[WARN] [{plugin_name}] ${{message}}`),
                error: (message) => console.error(`[ERROR] [{plugin_name}] ${{message}}`),
            }},

            // Configuration access
            getConfig: (key) => {{
                const config = {config_json};
                return config[key] || null;
            }},

            // Event bus (placeholder)
            events: {{
                publish: (eventType, data) => {{
                    console.log(`[EVENT] [{plugin_name}] Publishing: ${{eventType}}`);
                    return true;
                }},
                subscribe: (eventType, handler) => {{
                    console.log(`[EVENT] [{plugin_name}] Subscribing to: ${{eventType}}`);
                    return `sub_{plugin_name}_${{eventType}}`;
                }},
            }},
        }};

        // Override fetch to enforce network access control
        globalThis.fetch = async function(url, options) {{
            const urlStr = typeof url === 'string' ? url : url.toString();
            Ting.log.info('fetch: ' + urlStr);

            // Check if URL is allowed
            const allowedDomains = Ting.sandbox.allowedDomains;
            if (allowedDomains.length > 0) {{
                const domain = extractDomain(urlStr);
                const isAllowed = allowedDomains.some(pattern => domainMatches(domain, pattern));

                if (!isAllowed) {{
                    throw new Error(`Network access denied: ${{urlStr}}`);
                }}
            }}

            try {{
                Ting.log.info('calling op_fetch for ' + urlStr);
                const responseText = await Deno.core.ops.op_fetch(urlStr, options);
                Ting.log.info('op_fetch returned for ' + urlStr);
                return {{
                    ok: true,
                    status: 200,
                    statusText: "OK",
                    text: async () => responseText,
                    json: async () => JSON.parse(responseText),
                    headers: new Headers(),
                }};
            }} catch (e) {{
                Ting.log.error('op_fetch failed: ' + e);
                throw e;
            }}
        }};

        // Helper function to extract domain from URL
        function extractDomain(url) {{
            const matches = url.match(/^https?:\/\/([^/?#]+)(?:[/?#]|$)/i);
            return matches ? matches[1] : '';
        }}

        // Helper function to check if domain matches pattern (supports wildcards)
        function domainMatches(domain, pattern) {{
            if (pattern.startsWith('*.')) {{
                const base = pattern.substring(2);
                return domain.endsWith(base) || domain === base;
            }} else {{
                return domain === pattern;
            }}
        }}

        // Helper for invoking functions from Rust without recompiling scripts
        globalThis._ting_invoke = async function(funcName, args) {{
            try {{
                globalThis._ting_status = 'pending';
                globalThis._ting_result = undefined;
                globalThis._ting_error = undefined;

                const func = globalThis[funcName];
                if (typeof func !== 'function') {{
                    throw new Error(`Function ${{funcName}} not found`);
                }}

                const result = await func(args);
                globalThis._ting_result = JSON.stringify(result);
                globalThis._ting_status = 'success';
            }} catch (e) {{
                globalThis._ting_error = e.toString();
                globalThis._ting_status = 'error';
            }}
        }};
        "#,
        plugin_name = plugin_name,
        config_json = config_json,
        paths_json = paths_json,
        domains_json = domains_json,
    )
}
