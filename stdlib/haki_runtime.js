/**
 * haki_runtime.js — JavaScript host runtime for Haki WebAssembly modules.
 *
 * Provides the `env` import object that Haki Wasm modules expect, plus
 * helpers for loading and running compiled Haki programs in the browser
 * or Node.js.
 *
 * Usage (browser):
 *   <script type="module">
 *     import { loadHaki } from './haki_runtime.js';
 *     const haki = await loadHaki('myapp.wasm');
 *     haki.exports.main();
 *   </script>
 *
 * Usage (Node.js):
 *   const { loadHaki } = require('./haki_runtime.js');
 *   const haki = await loadHaki('myapp.wasm');
 *   haki.exports.main();
 */

// ── Memory helpers ────────────────────────────────────────────────────────────

/**
 * Read a null-terminated C string from Wasm linear memory.
 * Haki strings are UTF-8, null-terminated, stored as i32 pointers.
 */
function readString(memory, ptr) {
    if (!ptr) return '';
    const bytes = new Uint8Array(memory.buffer);
    let end = ptr;
    while (end < bytes.length && bytes[end] !== 0) end++;
    return new TextDecoder('utf-8').decode(bytes.slice(ptr, end));
}

/**
 * Write a JS string into Wasm linear memory, null-terminated.
 * Uses haki_alloc to allocate memory in Haki's heap.
 * Returns the pointer.
 */
function writeString(memory, allocFn, str) {
    const encoded = new TextEncoder().encode(str);
    const ptr = allocFn(encoded.length + 1);
    const bytes = new Uint8Array(memory.buffer);
    bytes.set(encoded, ptr);
    bytes[ptr + encoded.length] = 0;
    return ptr;
}

// ── Default env imports ───────────────────────────────────────────────────────
// These are the stdlib functions Haki Wasm modules import from "env".
// Override by passing a custom `env` object to loadHaki().

function makeDefaultEnv(memoryRef) {
    return {
        // ── Print functions ───────────────────────────────────────────────
        print(ptr) {
            console.log(readString(memoryRef.current, ptr));
        },
        print_int(n) {
            console.log(Number(n));
        },
        print_float(f) {
            console.log(f);
        },
        print_bool(b) {
            console.log(b !== 0);
        },

        // ── String operations ─────────────────────────────────────────────
        // These are no-ops in the default runtime — Haki's string operations
        // run in Wasm linear memory and don't need JS for simple cases.
        // Complex string ops call into these shims.
        string_concat(a_ptr, b_ptr) {
            const mem = memoryRef.current;
            const a = readString(mem, a_ptr);
            const b = readString(mem, b_ptr);
            // For now: write result back using the internal allocator
            // This requires haki_alloc to be exported — see loadHaki().
            const combined = a + b;
            if (memoryRef.alloc) {
                return writeString(mem, memoryRef.alloc, combined);
            }
            return 0;
        },
        string_length(ptr) {
            const mem = memoryRef.current;
            if (!ptr) return 0;
            const bytes = new Uint8Array(mem.buffer);
            let len = 0;
            while ((ptr + len) < bytes.length && bytes[ptr + len] !== 0) len++;
            return len;
        },
    };
}

// ── DOM bindings ──────────────────────────────────────────────────────────────
// Provided when std/dom.haki is imported. The Haki compiler emits calls to
// these via `extern "js"` declarations.

function makeDomEnv(memoryRef) {
    // Element handle table: maps i32 handles to DOM nodes.
    // Haki code holds i32 handles; the JS side resolves them.
    const elements = new Map();
    const eventHandlers = new Map();
    let nextHandle = 1;

    function storeElement(el) {
        const h = nextHandle++;
        elements.set(h, el);
        return h;
    }

    function getElement(handle) {
        return elements.get(handle) || null;
    }

    return {
        // ── Document ──────────────────────────────────────────────────────
        js_document_get_element_by_id(ptr, len) {
            const id = readString(memoryRef.current, ptr);
            const el = document.getElementById(id);
            return el ? storeElement(el) : 0;
        },

        js_document_create_element(tagPtr) {
            const tag = readString(memoryRef.current, tagPtr);
            const el = document.createElement(tag);
            return storeElement(el);
        },

        js_document_append_child(parentHandle, childHandle) {
            const parent = getElement(parentHandle);
            const child  = getElement(childHandle);
            if (parent && child) parent.appendChild(child);
        },

        js_document_body() {
            return storeElement(document.body);
        },

        // ── Element ───────────────────────────────────────────────────────
        js_element_get_text(handle) {
            const el = getElement(handle);
            if (!el) return 0;
            return memoryRef.alloc
                ? writeString(memoryRef.current, memoryRef.alloc, el.textContent || '')
                : 0;
        },

        js_element_set_text(handle, ptr) {
            const el = getElement(handle);
            if (el) el.textContent = readString(memoryRef.current, ptr);
        },

        js_element_set_html(handle, ptr) {
            const el = getElement(handle);
            if (el) el.innerHTML = readString(memoryRef.current, ptr);
        },

        js_element_set_attribute(handle, namePtr, valuePtr) {
            const el = getElement(handle);
            if (el) {
                const name  = readString(memoryRef.current, namePtr);
                const value = readString(memoryRef.current, valuePtr);
                el.setAttribute(name, value);
            }
        },

        js_element_get_attribute(handle, namePtr) {
            const el = getElement(handle);
            if (!el) return 0;
            const name = readString(memoryRef.current, namePtr);
            const val  = el.getAttribute(name) || '';
            return memoryRef.alloc
                ? writeString(memoryRef.current, memoryRef.alloc, val)
                : 0;
        },

        js_element_add_class(handle, classPtr) {
            const el = getElement(handle);
            if (el) el.classList.add(readString(memoryRef.current, classPtr));
        },

        js_element_remove_class(handle, classPtr) {
            const el = getElement(handle);
            if (el) el.classList.remove(readString(memoryRef.current, classPtr));
        },

        // ── Events ────────────────────────────────────────────────────────
        js_element_add_event_listener(handle, eventPtr, callbackFnIdx) {
            const el = getElement(handle);
            if (!el) return;
            const event = readString(memoryRef.current, eventPtr);
            const handler = () => {
                // Call back into Wasm: the callback is a Haki fn() -> void
                if (memoryRef.callFn) {
                    memoryRef.callFn(callbackFnIdx);
                }
            };
            // Store handler so it can be removed later
            const key = `${handle}:${event}:${callbackFnIdx}`;
            eventHandlers.set(key, handler);
            el.addEventListener(event, handler);
        },

        js_element_remove_event_listener(handle, eventPtr, callbackFnIdx) {
            const el = getElement(handle);
            if (!el) return;
            const event = readString(memoryRef.current, eventPtr);
            const key   = `${handle}:${event}:${callbackFnIdx}`;
            const handler = eventHandlers.get(key);
            if (handler) {
                el.removeEventListener(event, handler);
                eventHandlers.delete(key);
            }
        },

        // ── Window / timing ───────────────────────────────────────────────
        js_console_log(ptr) {
            console.log(readString(memoryRef.current, ptr));
        },

        js_window_set_timeout(callbackFnIdx, ms) {
            setTimeout(() => {
                if (memoryRef.callFn) memoryRef.callFn(callbackFnIdx);
            }, Number(ms));
        },

        js_window_request_animation_frame(callbackFnIdx) {
            requestAnimationFrame(() => {
                if (memoryRef.callFn) memoryRef.callFn(callbackFnIdx);
            });
        },

        // ── Fetch (async bridge) ──────────────────────────────────────────
        // Returns a request handle; response arrives via callback when ready.
        js_fetch(urlPtr, callbackFnIdx) {
            const url = readString(memoryRef.current, urlPtr);
            fetch(url)
                .then(r => r.text())
                .then(body => {
                    if (memoryRef.alloc && memoryRef.callFnWithArg) {
                        const ptr = writeString(memoryRef.current, memoryRef.alloc, body);
                        memoryRef.callFnWithArg(callbackFnIdx, ptr);
                    }
                })
                .catch(() => {
                    if (memoryRef.callFnWithArg) {
                        memoryRef.callFnWithArg(callbackFnIdx, 0);
                    }
                });
        },
    };
}

// ── Module loader ─────────────────────────────────────────────────────────────

/**
 * Load and instantiate a Haki Wasm module.
 *
 * @param {string|URL|Response|BufferSource} source
 *   In the browser: a URL string or fetch() Response.
 *   In Node.js: a file path string or Buffer.
 *
 * @param {object} [options]
 *   options.env      — additional imports to merge into the "env" object
 *   options.dom      — set to true to include DOM bindings (browser only)
 *   options.memory   — a pre-allocated WebAssembly.Memory instance
 *
 * @returns {Promise<{exports, memory, readString, writeString}>}
 */
async function loadHaki(source, options = {}) {
    // Mutable ref so closures in the env always see the current memory
    const memoryRef = { current: null, alloc: null, callFn: null, callFnWithArg: null };

    // Build the import object
    const env = {
        ...makeDefaultEnv(memoryRef),
        ...(options.dom !== false ? makeDomEnv(memoryRef) : {}),
        ...(options.env || {}),
    };

    const importObject = { env };

    // Instantiate
    let instance;
    if (typeof WebAssembly.instantiateStreaming === 'function' &&
        (typeof source === 'string' || source instanceof URL || source instanceof Response)) {
        // Browser streaming instantiation
        const fetchSource = typeof source === 'string' || source instanceof URL
            ? fetch(source)
            : Promise.resolve(source);
        const result = await WebAssembly.instantiateStreaming(fetchSource, importObject);
        instance = result.instance;
    } else {
        // Node.js / fallback: load as buffer
        let buffer;
        if (typeof source === 'string') {
            // Node.js file path
            const fs = await import('fs/promises');
            buffer = await fs.readFile(source);
        } else if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
            buffer = source;
        } else {
            buffer = await source.arrayBuffer();
        }
        const result = await WebAssembly.instantiate(buffer, importObject);
        instance = result.instance;
    }

    // Wire up memory reference — Haki exports 'memory'
    memoryRef.current = instance.exports.memory;

    // Wire up alloc if exported (haki_alloc for JS→Haki string passing)
    if (instance.exports.haki_alloc) {
        memoryRef.alloc = instance.exports.haki_alloc;
    }

    // Wire up call helpers for event callbacks
    if (instance.exports.__haki_call_fn) {
        memoryRef.callFn = instance.exports.__haki_call_fn;
    }
    if (instance.exports.__haki_call_fn_i32) {
        memoryRef.callFnWithArg = instance.exports.__haki_call_fn_i32;
    }

    return {
        exports: instance.exports,
        memory:  memoryRef.current,
        // Expose helpers for user code that needs to pass strings to Haki
        readString:  (ptr) => readString(memoryRef.current, ptr),
        writeString: (str) => writeString(memoryRef.current, memoryRef.alloc, str),
    };
}

// ── Exports ───────────────────────────────────────────────────────────────────

// Browser (ESM)
export { loadHaki, readString, writeString, makeDefaultEnv, makeDomEnv };

// Node.js (CJS fallback)
if (typeof module !== 'undefined') {
    module.exports = { loadHaki, readString, writeString, makeDefaultEnv, makeDomEnv };
}
