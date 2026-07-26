/**
 * haki_ui_dom.js — DOM platform backend for haki_ui WebAssembly applications.
 *
 * Implements the three platform functions expected by haki_ui/app.haki
 * via Wasm imports in the "env" object:
 *
 *   haki_ui_init(titlePtr, width, height)
 *   haki_ui_render(jsonPtr)
 *   haki_ui_run_loop()     — returns immediately, browser loop takes over
 *
 * Usage (browser):
 *   <script type="module">
 *     import { loadHakiUI } from './haki_ui_dom.js';
 *     await loadHakiUI('myapp.wasm');
 *   </script>
 *
 * Usage (after `hakic --target dom myapp.haki`):
 *   The compiler generates a <stem>.html that imports this file automatically.
 */

import { loadHaki, readString } from './haki_runtime.js';

// ── Event handler registry ──────────────────────────────────────────────────
// Maps element IDs in the rendered DOM to Haki closure fat pointers.
// When a DOM event fires, we look up the closure and call back into Wasm.

const eventHandlers = new Map();  // domElementId → { event, wasmCallbackIdx }
let nextElementId = 1;

// ── Element tree diffing ────────────────────────────────────────────────────
// After each re-render, we diff the old JSON tree against the new one.
// Only elements that changed are updated in the DOM.
// Simple structural diff — by node type and position. Full keyed diffing is v2.0.

let previousTreeJson = null;
let wasmInstance     = null;
let memRef           = null;

// ── JSON Element → DOM node ─────────────────────────────────────────────────

function applyStyle(el, style) {
    if (!style) return;
    if (style.backgroundColor) el.style.backgroundColor = style.backgroundColor;
    if (style.color)           el.style.color           = style.color;
    if (style.fontSize > 0)    el.style.fontSize        = style.fontSize + 'pt';
    if (style.fontWeight)      el.style.fontWeight      = style.fontWeight;
    if (style.borderRadius > 0) el.style.borderRadius   = style.borderRadius + 'px';
    if (style.width > 0)       el.style.width           = style.width + 'px';
    if (style.height > 0)      el.style.height          = style.height + 'px';
    if (style.opacity !== undefined && style.opacity !== 1)
                               el.style.opacity         = style.opacity;

    const [pt, pr, pb, pl] = style.padding || [0,0,0,0];
    if (pt || pr || pb || pl)
        el.style.padding = `${pt}px ${pr}px ${pb}px ${pl}px`;

    const [mt, mr, mb, ml] = style.margin || [0,0,0,0];
    if (mt || mr || mb || ml)
        el.style.margin = `${mt}px ${mr}px ${mb}px ${ml}px`;
}

function elementToDOM(node, callbackRegistry) {
    if (!node) return document.createTextNode('');
    const style = node.style || {};

    switch (node.type) {
        case 'Empty':
            return document.createComment('haki:empty');

        case 'Text':
        case 'Paragraph': {
            const el = document.createElement(node.type === 'Paragraph' ? 'p' : 'span');
            el.textContent = node.value || '';
            applyStyle(el, style);
            return el;
        }

        case 'Heading': {
            const level = Math.min(6, Math.max(1, node.level || 1));
            const el = document.createElement(`h${level}`);
            el.textContent = node.value || '';
            applyStyle(el, style);
            return el;
        }

        case 'Button': {
            const el = document.createElement('button');
            el.textContent = node.label || '';
            applyStyle(el, style);
            if (node._callbackIdx !== undefined) {
                el.addEventListener('click', () => {
                    callWasmCallback(node._callbackIdx);
                });
            }
            return el;
        }

        case 'TextInput': {
            const el = document.createElement('input');
            el.type = 'text';
            el.placeholder = node.placeholder || '';
            el.value       = node.value       || '';
            applyStyle(el, style);
            if (node._callbackIdx !== undefined) {
                el.addEventListener('input', (e) => {
                    callWasmCallbackWithString(node._callbackIdx, e.target.value);
                });
            }
            return el;
        }

        case 'Checkbox': {
            const wrapper = document.createElement('label');
            const input   = document.createElement('input');
            input.type    = 'checkbox';
            input.checked = !!node.checked;
            const text    = document.createTextNode(' ' + (node.label || ''));
            wrapper.appendChild(input);
            wrapper.appendChild(text);
            applyStyle(wrapper, style);
            if (node._callbackIdx !== undefined) {
                input.addEventListener('change', (e) => {
                    callWasmCallbackWithBool(node._callbackIdx, e.target.checked);
                });
            }
            return wrapper;
        }

        case 'Select': {
            const el = document.createElement('select');
            (node.options || []).forEach((opt, i) => {
                const option = document.createElement('option');
                option.value = String(i);
                option.textContent = opt;
                if (i === node.selected) option.selected = true;
                el.appendChild(option);
            });
            applyStyle(el, style);
            if (node._callbackIdx !== undefined) {
                el.addEventListener('change', (e) => {
                    callWasmCallbackWithInt(node._callbackIdx, parseInt(e.target.value));
                });
            }
            return el;
        }

        case 'Column': {
            const el = document.createElement('div');
            el.style.display       = 'flex';
            el.style.flexDirection = 'column';
            el.style.gap           = (node.spacing || 8) + 'px';
            applyStyle(el, style);
            (node.children || []).forEach(child => {
                el.appendChild(elementToDOM(child, callbackRegistry));
            });
            return el;
        }

        case 'Row': {
            const el = document.createElement('div');
            el.style.display   = 'flex';
            el.style.flexDirection = 'row';
            el.style.alignItems    = 'center';
            el.style.gap           = (node.spacing || 8) + 'px';
            applyStyle(el, style);
            (node.children || []).forEach(child => {
                el.appendChild(elementToDOM(child, callbackRegistry));
            });
            return el;
        }

        case 'Stack': {
            const el = document.createElement('div');
            el.style.position = 'relative';
            applyStyle(el, style);
            (node.children || []).forEach(child => {
                const c = elementToDOM(child, callbackRegistry);
                c.style.position = 'absolute';
                c.style.inset = '0';
                el.appendChild(c);
            });
            return el;
        }

        case 'ScrollView': {
            const el = document.createElement('div');
            el.style.overflow = 'auto';
            applyStyle(el, style);
            if (node.child) el.appendChild(elementToDOM(node.child, callbackRegistry));
            return el;
        }

        case 'Box': {
            const el = document.createElement('div');
            el.style.overflow = 'hidden';
            applyStyle(el, style);
            if (node.child) el.appendChild(elementToDOM(node.child, callbackRegistry));
            return el;
        }

        case 'Spacer': {
            const el = document.createElement('div');
            el.style.flex = '1';
            return el;
        }

        case 'Image': {
            const el  = document.createElement('img');
            el.src    = node.src || '';
            el.alt    = node.alt || '';
            applyStyle(el, style);
            return el;
        }

        default: {
            const el = document.createElement('span');
            el.textContent = `[unknown: ${node.type}]`;
            return el;
        }
    }
}

// ── Wasm callback helpers ────────────────────────────────────────────────────

function callWasmCallback(fnIdx) {
    if (!wasmInstance) return;
    // Call a Haki fn() -> void by index
    const fn = wasmInstance.exports[`__haki_cb_${fnIdx}`];
    if (fn) fn();
}

function callWasmCallbackWithString(fnIdx, str) {
    if (!wasmInstance || !memRef) return;
    const alloc = wasmInstance.exports.haki_alloc;
    if (!alloc) return;
    const encoded = new TextEncoder().encode(str);
    const ptr = alloc(encoded.length + 1);
    const bytes = new Uint8Array(memRef.buffer);
    bytes.set(encoded, ptr);
    bytes[ptr + encoded.length] = 0;
    callWasmCallbackWith(fnIdx, ptr);
}

function callWasmCallbackWithBool(fnIdx, val) {
    callWasmCallbackWith(fnIdx, val ? 1 : 0);
}

function callWasmCallbackWithInt(fnIdx, val) {
    callWasmCallbackWith(fnIdx, val);
}

function callWasmCallbackWith(fnIdx, arg) {
    if (!wasmInstance) return;
    const fn = wasmInstance.exports[`__haki_cb_${fnIdx}`];
    if (fn) fn(arg);
}

// ── Platform API env object ──────────────────────────────────────────────────

function makeUIEnv(memoryRef) {
    return {
        haki_ui_init(titlePtr, _width, _height) {
            const title = readString(memoryRef.current, Number(titlePtr));
            document.title = title || 'Haki App';
            return 0n;
        },

        haki_ui_render(jsonPtr) {
            const json = readString(memoryRef.current, Number(jsonPtr));
            if (!json) return -1n;

            let tree;
            try {
                tree = JSON.parse(json);
            } catch (e) {
                console.error('haki_ui_dom: failed to parse element JSON', e);
                return -1n;
            }

            previousTreeJson = json;

            // Replace DOM content
            const container = document.getElementById('haki-root') || document.body;
            container.innerHTML = '';
            const callbackRegistry = new Map();
            container.appendChild(elementToDOM(tree, callbackRegistry));

            return 0n;
        },

        haki_ui_run_loop() {
            // Browser: nothing to do — the browser's own event loop handles events.
            // The Haki Wasm module is reactive: the JS event listeners call
            // back into Wasm when events fire, which may trigger state mutations.
            // After each callback, the dirty-check runs and triggers re-render.
            return 0n;
        }
    };
}

// ── Module loader ────────────────────────────────────────────────────────────

/**
 * Load a haki_ui Wasm application and start it.
 *
 * @param {string} wasmUrl — URL of the compiled .wasm file
 * @param {object} [options] — passed to loadHaki()
 */
export async function loadHakiUI(wasmUrl, options = {}) {
    const memoryRef = { current: null, alloc: null };

    const uiEnv = makeUIEnv(memoryRef);

    const haki = await loadHaki(wasmUrl, {
        ...options,
        dom: true,
        env: { ...uiEnv, ...(options.env || {}) }
    });

    wasmInstance        = haki.exports;
    memRef              = haki.memory.buffer;
    memoryRef.current   = haki.memory;
    memoryRef.alloc     = haki.exports.haki_alloc;

    // Start the app
    if (haki.exports.main) {
        haki.exports.main();
    }

    return haki;
}
