import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  startClient(context);
  context.subscriptions.push(
    vscode.commands.registerCommand('musiclang.restartServer', async () => {
      await client?.stop();
      startClient(context);
      void vscode.window.showInformationMessage('MusicLang language server restarted.');
    }),
  );
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

function startClient(context: vscode.ExtensionContext): void {
  const serverCommand = resolveServerCommand(context);
  const serverOptions: ServerOptions = {
    command: serverCommand,
    options: {
      cwd: workspaceRoot(context),
    },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'musiclang' }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.music'),
    },
  };

  client = new LanguageClient('musiclang', 'MusicLang', serverOptions, clientOptions);
  context.subscriptions.push(client);
  void client.start();
}

function resolveServerCommand(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace.getConfiguration('musiclang').get<string>('serverPath');
  if (configured) {
    return configured;
  }

  const root = workspaceRoot(context);
  const candidates = [
    path.join(root, 'target', 'debug', executableName('musiclang-lsp')),
    path.join(root, 'target', 'release', executableName('musiclang-lsp')),
  ];
  const found = candidates.find((candidate) => fs.existsSync(candidate));
  if (found) {
    return found;
  }

  return 'musiclang-lsp';
}

function workspaceRoot(context: vscode.ExtensionContext): string {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? path.resolve(context.extensionPath, '..', '..');
}

function executableName(base: string): string {
  return process.platform === 'win32' ? `${base}.exe` : base;
}
