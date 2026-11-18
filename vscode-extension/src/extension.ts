// Starts the NOVA language server and connects it to the editor.
//
// This is the whole extension: everything the user sees — diagnostics,
// go-to-definition, find-all-references — is answered by `nova-lsp` over stdio.
// The extension's job is to find that binary, spawn it, and say something useful
// when it cannot.

import * as fs from "fs";

import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import { ExtensionContext, window, workspace } from "vscode";

let client: LanguageClient | undefined;

/// Resolve the configured path to something spawnable.
///
/// A bare name is left alone so the OS resolves it on `PATH`; an absolute path is
/// checked, because "spawn failed: ENOENT" from deep inside the client is a much
/// worse error message than the one below.
function resolveServer(configured: string): string | undefined {
  if (configured.includes("/") || configured.includes("\\")) {
    return fs.existsSync(configured) ? configured : undefined;
  }
  return configured;
}

export async function activate(context: ExtensionContext): Promise<void> {
  const configured = workspace
    .getConfiguration("nova")
    .get<string>("server.path", "nova-lsp");

  const command = resolveServer(configured);
  if (!command) {
    const message =
      `NOVA: the language server was not found at "${configured}". ` +
      `Build it with \`cargo build --release -p nova-lsp\` and set ` +
      `\`nova.server.path\` to the binary, or leave it as \`nova-lsp\` and put it on PATH.`;
    window.showErrorMessage(message);
    return;
  }

  const serverOptions: ServerOptions = {
    command,
    args: [],
    options: { env: { ...process.env } },
  };

  const clientOptions: LanguageClientOptions = {
    // `untitled` as well as `file`: a scratch buffer should get diagnostics too.
    documentSelector: [
      { scheme: "file", language: "nova" },
      { scheme: "untitled", language: "nova" },
    ],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.nova"),
    },
    outputChannel: window.createOutputChannel("NOVA Language Server"),
  };

  client = new LanguageClient(
    "nova",
    "NOVA Language Server",
    serverOptions,
    clientOptions,
  );

  try {
    await client.start();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    window.showErrorMessage(
      `NOVA: the language server failed to start (${reason}). ` +
        `Check that "${command}" runs and that it is a nova-lsp binary.`,
    );
    client = undefined;
    return;
  }

  context.subscriptions.push({
    dispose: () => {
      void client?.stop();
    },
  });
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
