/**
 * SchemaRefly VS Code Extension
 *
 * This extension provides schema contract verification for dbt projects.
 * It communicates with the schemarefly-lsp binary via the Language Server Protocol.
 */

import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as child_process from 'child_process';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
    Executable
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel;

/**
 * Activate the extension
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
    outputChannel = vscode.window.createOutputChannel('SchemaRefly');
    outputChannel.appendLine('SchemaRefly extension activating...');

    // Check if we're in a dbt project
    const dbtProjectPath = await findDbtProject();
    if (!dbtProjectPath) {
        outputChannel.appendLine('No dbt project found in workspace. Extension will activate when dbt_project.yml is detected.');
        // Still register commands but don't start the server
        registerCommands(context);
        return;
    }

    outputChannel.appendLine(`Found dbt project at: ${dbtProjectPath}`);

    // Register commands
    registerCommands(context);

    // Start the language server
    await startLanguageServer(context, dbtProjectPath);

    outputChannel.appendLine('SchemaRefly extension activated successfully!');
}

/**
 * Deactivate the extension
 */
export async function deactivate(): Promise<void> {
    if (client) {
        await client.stop();
    }
}

/**
 * Find the dbt project root in the workspace
 */
async function findDbtProject(): Promise<string | undefined> {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders) {
        return undefined;
    }

    for (const folder of workspaceFolders) {
        // Check root
        const rootDbtProject = path.join(folder.uri.fsPath, 'dbt_project.yml');
        if (fs.existsSync(rootDbtProject)) {
            return folder.uri.fsPath;
        }

        // Check subdirectories (one level deep)
        try {
            const entries = fs.readdirSync(folder.uri.fsPath, { withFileTypes: true });
            for (const entry of entries) {
                if (entry.isDirectory()) {
                    const subDbtProject = path.join(folder.uri.fsPath, entry.name, 'dbt_project.yml');
                    if (fs.existsSync(subDbtProject)) {
                        return path.join(folder.uri.fsPath, entry.name);
                    }
                }
            }
        } catch (e) {
            // Ignore errors reading directories
        }
    }

    return undefined;
}

/**
 * Register extension commands
 */
function registerCommands(context: vscode.ExtensionContext): void {
    // Restart Language Server command
    context.subscriptions.push(
        vscode.commands.registerCommand('schemarefly.restart', async () => {
            if (client) {
                outputChannel.appendLine('Restarting SchemaRefly language server...');
                await client.stop();
                const dbtProjectPath = await findDbtProject();
                if (dbtProjectPath) {
                    await startLanguageServer(context, dbtProjectPath);
                    vscode.window.showInformationMessage('SchemaRefly language server restarted.');
                } else {
                    vscode.window.showWarningMessage('No dbt project found.');
                }
            } else {
                vscode.window.showWarningMessage('SchemaRefly language server is not running.');
            }
        })
    );

    // Check Contracts command
    context.subscriptions.push(
        vscode.commands.registerCommand('schemarefly.check', async () => {
            const dbtProjectPath = await findDbtProject();
            if (!dbtProjectPath) {
                vscode.window.showWarningMessage('No dbt project found in workspace.');
                return;
            }

            // Run schemarefly check in terminal
            const terminal = vscode.window.createTerminal('SchemaRefly Check');
            terminal.show();
            terminal.sendText(`cd "${dbtProjectPath}" && schemarefly check --verbose`);
        })
    );

    // Show Output command
    context.subscriptions.push(
        vscode.commands.registerCommand('schemarefly.showOutput', () => {
            outputChannel.show();
        })
    );
}

/**
 * Start the language server
 */
async function startLanguageServer(
    context: vscode.ExtensionContext,
    dbtProjectPath: string
): Promise<void> {
    // Get server path from configuration or find it
    const config = vscode.workspace.getConfiguration('schemarefly');
    let serverPath = config.get<string>('serverPath');

    if (!serverPath || serverPath === '') {
        serverPath = await findServerBinary(context);
    }

    if (!serverPath) {
        const selection = await vscode.window.showErrorMessage(
            'SchemaRefly language server not found. Please install it or configure the path.',
            'Install Instructions',
            'Configure Path'
        );

        if (selection === 'Install Instructions') {
            vscode.env.openExternal(vscode.Uri.parse('https://github.com/owner/schemarefly#installation'));
        } else if (selection === 'Configure Path') {
            vscode.commands.executeCommand('workbench.action.openSettings', 'schemarefly.serverPath');
        }
        return;
    }

    outputChannel.appendLine(`Using language server at: ${serverPath}`);

    // Verify the server exists and is executable
    if (!fs.existsSync(serverPath)) {
        vscode.window.showErrorMessage(`SchemaRefly server not found at: ${serverPath}`);
        return;
    }

    // Server options
    const serverExecutable: Executable = {
        command: serverPath,
        args: [],
        options: {
            cwd: dbtProjectPath,
            env: {
                ...process.env,
                RUST_LOG: config.get<string>('trace.server') === 'verbose' ? 'debug' : 'info'
            }
        }
    };

    const serverOptions: ServerOptions = {
        run: serverExecutable,
        debug: serverExecutable
    };

    // Client options
    const clientOptions: LanguageClientOptions = {
        // Document selector for SQL files
        documentSelector: [
            { scheme: 'file', language: 'sql' },
            { scheme: 'file', language: 'jinja-sql' },
            { scheme: 'file', pattern: '**/*.sql' }
        ],
        synchronize: {
            // Watch for changes to schemarefly.toml and manifest.json
            fileEvents: [
                vscode.workspace.createFileSystemWatcher('**/schemarefly.toml'),
                vscode.workspace.createFileSystemWatcher('**/target/manifest.json'),
                vscode.workspace.createFileSystemWatcher('**/dbt_project.yml')
            ]
        },
        outputChannel: outputChannel,
        traceOutputChannel: outputChannel,
        workspaceFolder: {
            uri: vscode.Uri.file(dbtProjectPath),
            name: path.basename(dbtProjectPath),
            index: 0
        }
    };

    // Create the language client
    client = new LanguageClient(
        'schemarefly',
        'SchemaRefly Language Server',
        serverOptions,
        clientOptions
    );

    // Register client for disposal
    context.subscriptions.push(client);

    // Start the client (which also starts the server)
    try {
        await client.start();
        outputChannel.appendLine('Language server started successfully.');

        // Show status bar item
        const statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Right,
            100
        );
        statusBarItem.text = '$(database) SchemaRefly';
        statusBarItem.tooltip = 'SchemaRefly is active';
        statusBarItem.command = 'schemarefly.showOutput';
        statusBarItem.show();
        context.subscriptions.push(statusBarItem);

        // Check for manifest.json
        const manifestPath = path.join(dbtProjectPath, 'target', 'manifest.json');
        if (!fs.existsSync(manifestPath)) {
            vscode.window.showWarningMessage(
                'No manifest.json found. Run "dbt compile" to enable full diagnostics.',
                'Run dbt compile'
            ).then(selection => {
                if (selection === 'Run dbt compile') {
                    const terminal = vscode.window.createTerminal('dbt compile');
                    terminal.show();
                    terminal.sendText(`cd "${dbtProjectPath}" && dbt compile`);
                }
            });
        }

    } catch (error) {
        outputChannel.appendLine(`Failed to start language server: ${error}`);
        vscode.window.showErrorMessage(`Failed to start SchemaRefly: ${error}`);
    }
}

/**
 * Find the schemarefly-lsp binary
 */
async function findServerBinary(context: vscode.ExtensionContext): Promise<string | undefined> {
    // Check order:
    // 1. Bundled binary in extension
    // 2. Binary in PATH
    // 3. Common install locations

    // 1. Check bundled binary
    const bundledPaths = getPlatformBinaryPaths(context);
    for (const bundledPath of bundledPaths) {
        if (fs.existsSync(bundledPath)) {
            outputChannel.appendLine(`Found bundled binary at: ${bundledPath}`);
            return bundledPath;
        }
    }

    // 2. Check PATH
    const pathBinary = findInPath('schemarefly-lsp');
    if (pathBinary) {
        outputChannel.appendLine(`Found binary in PATH: ${pathBinary}`);
        return pathBinary;
    }

    // Also check for the CLI which might have the LSP mode
    const cliBinary = findInPath('schemarefly');
    if (cliBinary) {
        // Check if it's a recent version that includes LSP
        // For now, we need the dedicated LSP binary
        outputChannel.appendLine(`Found CLI binary at ${cliBinary}, but LSP binary is required.`);
    }

    // 3. Check common install locations
    const commonPaths = getCommonInstallPaths();
    for (const commonPath of commonPaths) {
        if (fs.existsSync(commonPath)) {
            outputChannel.appendLine(`Found binary at common location: ${commonPath}`);
            return commonPath;
        }
    }

    outputChannel.appendLine('Could not find schemarefly-lsp binary.');
    return undefined;
}

/**
 * Get platform-specific bundled binary paths
 */
function getPlatformBinaryPaths(context: vscode.ExtensionContext): string[] {
    const platform = os.platform();
    const arch = os.arch();
    const extensionPath = context.extensionPath;

    const paths: string[] = [];

    // Platform-specific binary name
    let binaryName = 'schemarefly-lsp';
    if (platform === 'win32') {
        binaryName = 'schemarefly-lsp.exe';
    }

    // Bundled in extension directory
    paths.push(path.join(extensionPath, 'bin', binaryName));
    paths.push(path.join(extensionPath, 'server', binaryName));

    // Platform-specific subdirectory
    let platformDir = '';
    if (platform === 'darwin') {
        platformDir = arch === 'arm64' ? 'darwin-arm64' : 'darwin-x64';
    } else if (platform === 'linux') {
        platformDir = arch === 'arm64' ? 'linux-arm64' : 'linux-x64';
    } else if (platform === 'win32') {
        platformDir = 'win32-x64';
    }

    if (platformDir) {
        paths.push(path.join(extensionPath, 'bin', platformDir, binaryName));
        paths.push(path.join(extensionPath, 'server', platformDir, binaryName));
    }

    return paths;
}

/**
 * Get common installation paths for the binary
 */
function getCommonInstallPaths(): string[] {
    const paths: string[] = [];
    const platform = os.platform();
    const homeDir = os.homedir();

    if (platform === 'darwin' || platform === 'linux') {
        paths.push('/usr/local/bin/schemarefly-lsp');
        paths.push('/usr/bin/schemarefly-lsp');
        paths.push(path.join(homeDir, '.local', 'bin', 'schemarefly-lsp'));
        paths.push(path.join(homeDir, '.cargo', 'bin', 'schemarefly-lsp'));
    } else if (platform === 'win32') {
        paths.push(path.join(homeDir, '.cargo', 'bin', 'schemarefly-lsp.exe'));
        paths.push('C:\\Program Files\\schemarefly\\schemarefly-lsp.exe');
    }

    return paths;
}

/**
 * Find a binary in PATH
 */
function findInPath(binaryName: string): string | undefined {
    const platform = os.platform();
    const isWindows = platform === 'win32';

    // Add .exe extension on Windows
    if (isWindows && !binaryName.endsWith('.exe')) {
        binaryName += '.exe';
    }

    try {
        const result = child_process.execSync(
            isWindows ? `where ${binaryName}` : `which ${binaryName}`,
            { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
        );
        const foundPath = result.trim().split('\n')[0];
        if (foundPath && fs.existsSync(foundPath)) {
            return foundPath;
        }
    } catch {
        // Not found in PATH
    }

    return undefined;
}
