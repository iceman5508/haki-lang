/**
 * Haki VS Code Extension v0.3.0
 *
 * Activates when a .haki file is opened. Spawns `hakic lsp` as a subprocess
 * and connects it to the VS Code Language Client, which handles all the
 * JSON-RPC communication automatically.
 *
 * Features provided by hakic-lsp (v3.5+):
 *   - Inline diagnostics (parse errors + type errors)
 *   - Hover: shows the type of any expression
 *   - Go-to-definition: jumps to function/struct/class/enum/annotation definitions
 *   - Completions: f-strings, @annotations, Map iteration, channels
 *   - Signature help: try expressions, annotation params
 *   - Find references + rename: across modules
 *   - Document symbols: includes annotation definitions
 *
 * New in v3.5:
 *   - @annotation syntax highlighting and completion
 *   - f"string {interpolation}" support
 *   - `for k, v in map` snippet
 *   - `annotation @name(params) { }` snippet
 *   - `try expr` snippet
 */

import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
    Executable,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    const config = vscode.workspace.getConfiguration('haki');
    const serverPath = config.get<string>('server.path') ?? 'hakic';

    // Server: spawn `hakic lsp` — it reads/writes JSON-RPC on stdio.
    const serverExecutable: Executable = {
        command: serverPath,
        args: ['lsp'],
        options: {
            // Inherit the workspace folder as the working directory so that
            // relative imports in .haki files resolve correctly.
            cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
        },
        transport: TransportKind.stdio,
    };

    const serverOptions: ServerOptions = {
        run:   serverExecutable,
        debug: serverExecutable,   // same binary, no debug flags needed
    };

    // Client: tell VS Code which files to send to the server.
    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'haki' },
        ],
        synchronize: {
            // Re-send the file whenever it's saved (in addition to on-change).
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.haki'),
        },
        traceOutputChannel: vscode.window.createOutputChannel('Haki LSP Trace'),
    };

    client = new LanguageClient(
        'hakic-lsp',
        'Haki Language Server',
        serverOptions,
        clientOptions,
    );

    client.start();

    // Register a "Restart Language Server" command — useful when hakic is
    // recompiled and the user wants to pick up the new binary without
    // reloading the entire VS Code window.
    context.subscriptions.push(
        vscode.commands.registerCommand('haki.restartServer', async () => {
            if (client) {
                await client.stop();
                await client.start();
                vscode.window.showInformationMessage('Haki language server restarted.');
            }
        })
    );

    context.subscriptions.push(client);
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}
