"use strict";
/**
 * Haki VS Code Extension
 *
 * Activates when a .haki file is opened. Spawns `hakic lsp` as a subprocess
 * and connects it to the VS Code Language Client, which handles all the
 * JSON-RPC communication automatically.
 *
 * Features provided by hakic-lsp:
 *   - Inline diagnostics (parse errors + type errors)
 *   - Hover: shows the type of any expression
 *   - Go-to-definition: jumps to function/struct/class/enum definitions
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
let client;
function activate(context) {
    const config = vscode.workspace.getConfiguration('haki');
    const serverPath = config.get('server.path') ?? 'hakic';
    // Server: spawn `hakic lsp` — it reads/writes JSON-RPC on stdio.
    const serverExecutable = {
        command: serverPath,
        args: ['lsp'],
        options: {
            // Inherit the workspace folder as the working directory so that
            // relative imports in .haki files resolve correctly.
            cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
        },
        transport: node_1.TransportKind.stdio,
    };
    const serverOptions = {
        run: serverExecutable,
        debug: serverExecutable, // same binary, no debug flags needed
    };
    // Client: tell VS Code which files to send to the server.
    const clientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'haki' },
        ],
        synchronize: {
            // Re-send the file whenever it's saved (in addition to on-change).
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.haki'),
        },
        traceOutputChannel: vscode.window.createOutputChannel('Haki LSP Trace'),
    };
    client = new node_1.LanguageClient('hakic-lsp', 'Haki Language Server', serverOptions, clientOptions);
    client.start();
    // Register a "Restart Language Server" command — useful when hakic is
    // recompiled and the user wants to pick up the new binary without
    // reloading the entire VS Code window.
    context.subscriptions.push(vscode.commands.registerCommand('haki.restartServer', async () => {
        if (client) {
            await client.stop();
            await client.start();
            vscode.window.showInformationMessage('Haki language server restarted.');
        }
    }));
    context.subscriptions.push(client);
}
function deactivate() {
    return client?.stop();
}
//# sourceMappingURL=extension.js.map