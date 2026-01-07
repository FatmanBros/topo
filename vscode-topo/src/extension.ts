import * as path from 'path';
import * as fs from 'fs';
import { workspace, ExtensionContext, window } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
    // Find topo-lsp binary
    const serverPath = findServerPath(context);

    if (!serverPath) {
        window.showErrorMessage(
            'Topo LSP server not found. Please install topo or set topo.lsp.path in settings.'
        );
        return;
    }

    // Server options - run the topo-lsp binary
    const serverOptions: ServerOptions = {
        run: {
            command: serverPath,
            transport: TransportKind.stdio,
        },
        debug: {
            command: serverPath,
            transport: TransportKind.stdio,
        },
    };

    // Client options
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'topo' }],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/*.tp'),
        },
    };

    // Create and start the client
    client = new LanguageClient(
        'topoLanguageServer',
        'Topo Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client
    client.start();

    console.log('Topo extension activated');
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

function findServerPath(context: ExtensionContext): string | null {
    // 1. Check user settings
    const config = workspace.getConfiguration('topo');
    const configPath = config.get<string>('lsp.path');
    if (configPath && fs.existsSync(configPath)) {
        return configPath;
    }

    // 2. Check bundled binary in extension
    const bundledPaths = [
        path.join(context.extensionPath, 'bin', 'topo-lsp'),
        path.join(context.extensionPath, 'bin', 'topo-lsp.exe'),
    ];
    for (const p of bundledPaths) {
        if (fs.existsSync(p)) {
            return p;
        }
    }

    // 3. Check workspace target directory (for development)
    const workspaceFolders = workspace.workspaceFolders;
    if (workspaceFolders) {
        for (const folder of workspaceFolders) {
            const devPaths = [
                path.join(folder.uri.fsPath, 'target', 'release', 'topo-lsp'),
                path.join(folder.uri.fsPath, 'target', 'debug', 'topo-lsp'),
            ];
            for (const p of devPaths) {
                if (fs.existsSync(p)) {
                    return p;
                }
            }
        }
    }

    // 4. Check PATH
    const pathEnv = process.env.PATH || '';
    const pathDirs = pathEnv.split(path.delimiter);
    for (const dir of pathDirs) {
        const candidates = [
            path.join(dir, 'topo-lsp'),
            path.join(dir, 'topo-lsp.exe'),
        ];
        for (const p of candidates) {
            if (fs.existsSync(p)) {
                return p;
            }
        }
    }

    // 5. Check common install locations
    const commonPaths = [
        path.join(process.env.HOME || '', '.cargo', 'bin', 'topo-lsp'),
        '/usr/local/bin/topo-lsp',
        '/usr/bin/topo-lsp',
    ];
    for (const p of commonPaths) {
        if (fs.existsSync(p)) {
            return p;
        }
    }

    return null;
}
