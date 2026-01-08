# VS Code Extension Development Guide

Complete guide for running, testing, and developing the SchemaRefly VS Code extension.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Detailed Setup](#detailed-setup)
- [Testing Features](#testing-features)
- [Debugging](#debugging)
- [Making Changes](#making-changes)
- [Common Issues](#common-issues)
- [Project Structure](#project-structure)

---

## Prerequisites

Before you begin, ensure you have:

- ✅ **Node.js** (v16 or later) and **npm** installed
- ✅ **VS Code** installed on your system
- ✅ **Rust toolchain** (for building the LSP server)
- ✅ **Git** (for version control)

### Verify Prerequisites

```bash
# Check Node.js version
node --version  # Should be v16+

# Check npm version
npm --version

# Check Rust
cargo --version

# Check VS Code
code --version
```

---

## Quick Start

The fastest way to run the extension in development mode:

### 1. Navigate to Extension Directory

```bash
cd /path/to/SchemaRefly/editors/vscode
```

### 2. Install Dependencies

```bash
npm install
```

This installs all TypeScript dependencies including `vscode-languageclient`.

### 3. Build LSP Server

The extension needs the `schemarefly-lsp` binary to function:

```bash
# From the SchemaRefly root directory
cd /path/to/SchemaRefly
cargo build --bin schemarefly-lsp

# The binary will be at: target/debug/schemarefly-lsp
```

### 4. Open Extension in VS Code

```bash
cd editors/vscode
code .
```

### 5. Launch Extension Development Host

- Press **F5** (or Run → Start Debugging)
- A new VS Code window opens titled **[Extension Development Host]**
- The extension is now running in development mode!

### 6. Open a dbt Project

In the Extension Development Host window:
- File → Open Folder
- Select a folder containing `dbt_project.yml`
- The extension will auto-activate

---

## Detailed Setup

### Step 1: Clone and Navigate

```bash
git clone <repository-url>
cd SchemaRefly/editors/vscode
```

### Step 2: Install npm Dependencies

```bash
npm install
```

This installs:
- `vscode-languageclient` (^9.0.1) - LSP client library
- TypeScript compiler and type definitions
- ESLint for code linting
- Other development dependencies

### Step 3: Compile TypeScript

```bash
npm run compile
```

This compiles `src/extension.ts` to JavaScript in the `out/` directory.

**Output:**
```
> schemarefly@0.1.0 compile
> tsc -p ./
```

### Step 4: Build the LSP Server

```bash
# From SchemaRefly root
cargo build --bin schemarefly-lsp

# Or for optimized build
cargo build --release --bin schemarefly-lsp
```

**Binary locations:**
- Debug: `target/debug/schemarefly-lsp`
- Release: `target/release/schemarefly-lsp`

### Step 5: Configure LSP Binary Path (Optional)

The extension auto-discovers the LSP binary, but you can configure it:

**Option A: Add to PATH**
```bash
export PATH="/path/to/SchemaRefly/target/debug:$PATH"
```

**Option B: Symlink to system location**
```bash
sudo ln -s /path/to/SchemaRefly/target/debug/schemarefly-lsp /usr/local/bin/
```

**Option C: Configure in VS Code settings**

In your workspace settings (`.vscode/settings.json`):
```json
{
  "schemarefly.serverPath": "/path/to/SchemaRefly/target/debug/schemarefly-lsp"
}
```

### Step 6: Launch Extension

From `editors/vscode` folder in VS Code:

1. **Open the folder**: `code .`
2. **Press F5** or use the Debug panel:
   - Click "Run and Debug" in the left sidebar
   - Select "Run Extension" configuration
   - Click the green play button ▶️

---

## Testing Features

### 1. Test Extension Activation

**Expected behavior when opening a dbt project:**

✅ **Status Bar**: Shows "$(database) SchemaRefly" in the bottom-right
✅ **Output Panel**: Shows activation messages
✅ **LSP Server**: Starts automatically

**Check Output Panel:**
1. View → Output
2. Select "SchemaRefly" from dropdown
3. Look for:
   ```
   SchemaRefly extension activating...
   Found dbt project at: /path/to/project
   Using language server at: /path/to/schemarefly-lsp
   Language server started successfully.
   SchemaRefly LSP initialized
   ```

### 2. Test Real-Time Diagnostics

**Prerequisites:**
- dbt project must have `target/manifest.json` (run `dbt compile`)
- Open a SQL model file from `models/` directory

**Test:**
1. Open a `.sql` file
2. Save the file (Cmd+S / Ctrl+S)
3. Check for diagnostics (squiggly lines) if there are contract violations

**Example Contract Violation:**
```sql
-- If contract expects columns [id, name]
-- but SQL returns [id, email]
SELECT id, email FROM users
```

You should see a diagnostic error on the SQL.

### 3. Test Hover Provider

**Test:**
1. Open a SQL model file
2. Hover over SQL code (e.g., column names, table references)
3. Should display inferred schema in a popup:

**Expected Popup:**
```markdown
## Inferred Schema

| Column | Type |
|--------|------|
| id     | INT64 |
| name   | STRING |
| email  | STRING |
```

### 4. Test Go-to-Definition

**Test:**
1. Place cursor on a `ref('model_name')` call
2. Right-click → Go to Definition (or F12)
3. Should jump to the model's SQL file

**Note:** Full implementation may require cursor position parsing (currently placeholder).

### 5. Test Commands

Open Command Palette (Cmd+Shift+P / Ctrl+Shift+P) and test:

#### **SchemaRefly: Restart Language Server**
- Stops and restarts the LSP server
- Useful after making changes to LSP code

#### **SchemaRefly: Check Contracts**
- Opens integrated terminal
- Runs `schemarefly check --verbose`

#### **SchemaRefly: Show Output**
- Opens the Output panel
- Shows SchemaRefly extension logs

### 6. Test Configuration

Open Settings (Cmd+, / Ctrl+,) and search for "schemarefly":

**Available Settings:**

| Setting | Default | Description |
|---------|---------|-------------|
| `schemarefly.serverPath` | `""` | Path to LSP binary |
| `schemarefly.trace.server` | `"off"` | LSP trace level |
| `schemarefly.diagnostics.onSave` | `true` | Run diagnostics on save |
| `schemarefly.diagnostics.onType` | `true` | Run diagnostics as you type |
| `schemarefly.hover.showInferredSchema` | `true` | Show schema on hover |

**Test Configuration Changes:**
1. Disable `diagnostics.onType`
2. Type invalid SQL
3. Verify diagnostics only appear on save

---

## Debugging

### View Extension Logs

**Output Panel (Extension Side):**
1. In Extension Development Host window
2. View → Output
3. Select "SchemaRefly" from dropdown
4. Shows extension activation, LSP communication

**Debug Console (VS Code Side):**
1. In main VS Code window (not Extension Development Host)
2. View → Debug Console
3. Shows TypeScript runtime logs and errors

### Enable LSP Trace

Set trace level to `verbose`:

**In Extension Development Host Settings:**
```json
{
  "schemarefly.trace.server": "verbose"
}
```

**Output shows:**
```
[Trace - 10:30:15] Sending request 'initialize - (1)'
[Trace - 10:30:15] Received response 'initialize - (1)'
```

### Debug LSP Server

**Run LSP server manually:**
```bash
# From dbt project directory
cd /path/to/dbt-project

# Set log level
export RUST_LOG=debug

# Run LSP server (communicates via stdin/stdout)
/path/to/schemarefly-lsp
```

**Test LSP communication:**
```bash
# Send LSP initialize request
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | /path/to/schemarefly-lsp
```

### Set Breakpoints in Extension Code

1. Open `src/extension.ts` in VS Code
2. Click left of line number to set breakpoint (red dot)
3. Press F5 to launch with debugger
4. Execution pauses at breakpoints

**Useful breakpoints:**
- `activate()` - Extension activation
- `startLanguageServer()` - LSP server start
- `findServerBinary()` - Binary discovery
- Command handlers - `schemarefly.restart`, etc.

### Common Debug Scenarios

#### **Extension Not Activating**

Check:
1. Is `dbt_project.yml` present in opened folder?
2. Check Output panel for errors
3. Verify `activationEvents` in `package.json`

#### **LSP Server Not Found**

Check:
1. Binary exists: `ls -lh /path/to/schemarefly-lsp`
2. Binary is executable: `chmod +x /path/to/schemarefly-lsp`
3. Check extension logs for binary search paths

#### **No Diagnostics Appearing**

Check:
1. `target/manifest.json` exists (run `dbt compile`)
2. File is a SQL model in dbt project
3. Configuration: `diagnostics.onSave` is enabled
4. Check Output panel for LSP errors

---

## Making Changes

### Development Workflow

#### **1. Edit Extension Code**

```bash
# Make changes to:
editors/vscode/src/extension.ts
editors/vscode/package.json
```

#### **2. Compile TypeScript**

**Option A: Watch mode (auto-compile on save)**
```bash
npm run watch
```
Leave this running in a terminal.

**Option B: Manual compile**
```bash
npm run compile
```

#### **3. Reload Extension**

In Extension Development Host window:
- **Cmd+R** (macOS) or **Ctrl+R** (Windows/Linux)
- Or click "Restart" icon in debug toolbar
- Extension reloads with your changes

#### **4. Test Changes**

Re-test the features you modified.

### Modifying LSP Server

If you make changes to `crates/schemarefly-lsp/`:

**1. Rebuild LSP binary:**
```bash
cargo build --bin schemarefly-lsp
```

**2. Restart LSP server in extension:**
- Command Palette → "SchemaRefly: Restart Language Server"

### Linting Code

```bash
npm run lint
```

Runs ESLint on TypeScript code. Fix issues before committing.

### Running Tests

```bash
npm test
```

Runs extension tests (if configured).

---

## Common Issues

### Issue: "SchemaRefly server not found"

**Solution:**
1. Build LSP binary: `cargo build --bin schemarefly-lsp`
2. Ensure binary is in PATH or configure `serverPath`
3. Check extension output for binary search paths

### Issue: "No dbt project found in workspace"

**Solution:**
1. Open a folder containing `dbt_project.yml` file
2. Extension only activates in dbt projects
3. Check if file is at root or in subdirectory (extension checks 1 level deep)

### Issue: "No manifest.json found"

**Solution:**
1. Run `dbt compile` in your dbt project
2. This generates `target/manifest.json`
3. LSP server needs manifest for diagnostics

### Issue: Extension doesn't activate

**Solution:**
1. Check Output panel (View → Output → SchemaRefly) for errors
2. Verify `dbt_project.yml` exists in opened folder
3. Try reloading window (Cmd+R / Ctrl+R)

### Issue: Changes not reflected after reload

**Solution:**
1. Ensure you ran `npm run compile` after changes
2. Check `out/` directory has updated `.js` files
3. Try closing and reopening Extension Development Host (not just reload)

### Issue: TypeScript compilation errors

**Solution:**
```bash
# Clean and rebuild
rm -rf out/
npm run compile
```

### Issue: LSP server crashes

**Check LSP logs:**
1. The LSP server logs to stderr
2. Check Output panel for error messages
3. Run server manually with `RUST_LOG=debug` for detailed logs

### Issue: Performance issues with diagnostics

**Solution:**
1. Disable `diagnostics.onType` - only run on save
2. For large files, LSP may be slow
3. Check CPU usage of `schemarefly-lsp` process

---

## Project Structure

```
editors/vscode/
├── src/
│   └── extension.ts              # Main extension code
│       ├── activate()            # Extension entry point
│       ├── findDbtProject()      # Auto-detect dbt projects
│       ├── startLanguageServer() # Start LSP client
│       ├── findServerBinary()    # Locate LSP binary
│       └── registerCommands()    # Register VS Code commands
│
├── out/                          # Compiled JavaScript (generated)
│   └── extension.js
│
├── node_modules/                 # npm dependencies
│
├── .vscode/
│   ├── launch.json              # Debug configurations
│   │   └── "Run Extension"      # F5 launches this
│   └── tasks.json               # Build tasks
│       └── "npm: watch"         # Auto-compile task
│
├── package.json                  # Extension manifest
│   ├── name: "schemarefly"
│   ├── activationEvents         # When to activate
│   ├── contributes             # Extension contributions
│   │   ├── configuration       # Settings
│   │   ├── commands            # Commands
│   │   └── languages           # Language support
│   └── scripts                 # npm scripts
│
├── tsconfig.json                # TypeScript config
├── .eslintrc.json               # ESLint config
├── language-configuration.json  # Jinja SQL language
├── .vscodeignore               # Files excluded from package
├── .gitignore                  # Git exclusions
├── README.md                   # User documentation
└── DEVELOPMENT.md              # This file
```

### Key Files

#### `src/extension.ts`

**Main Functions:**

- `activate()` - Called when extension activates
- `deactivate()` - Called when extension deactivates
- `findDbtProject()` - Searches for `dbt_project.yml`
- `startLanguageServer()` - Creates and starts LSP client
- `findServerBinary()` - Discovers LSP binary location
- `registerCommands()` - Registers extension commands

**LSP Client Setup:**
```typescript
const client = new LanguageClient(
  'schemarefly',                  // ID
  'SchemaRefly Language Server',   // Name
  serverOptions,                   // How to start server
  clientOptions                    // Document filters, watchers
);

await client.start();
```

#### `package.json`

**Extension Manifest:**

- **activationEvents**: When extension activates
  - `workspaceContains:dbt_project.yml`
  - `workspaceContains:**/dbt_project.yml`

- **contributes.configuration**: Extension settings
- **contributes.commands**: Extension commands
- **contributes.languages**: Language definitions

#### `.vscode/launch.json`

**Debug Configuration:**

- **Run Extension**: Launches Extension Development Host
- **preLaunchTask**: Compiles TypeScript before launch

---

## Testing Without a dbt Project

Use the SchemaRefly fixtures:

```bash
cd /path/to/SchemaRefly/fixtures/mini-dbt-project

# Compile to generate manifest
dbt compile

# Open in Extension Development Host
# File → Open Folder → Select this directory
```

The fixture project includes:
- `dbt_project.yml`
- Sample models in `models/`
- `target/manifest.json` (after compile)

---

## Advanced Topics

### Debugging LSP Protocol

**Enable verbose tracing:**
```json
{
  "schemarefly.trace.server": "verbose"
}
```

**LSP message flow:**
```
Extension (Client)  <-->  LSP Server
     |                        |
     |-- initialize() ------->|
     |<----- capabilities ----|
     |                        |
     |-- textDocument/didOpen |
     |-- textDocument/didSave |
     |<-- publishDiagnostics --|
     |                        |
     |-- textDocument/hover ->|
     |<----- hover content ---|
```

### Custom LSP Binary Path

**Per-workspace configuration:**

`.vscode/settings.json` in your dbt project:
```json
{
  "schemarefly.serverPath": "/custom/path/to/schemarefly-lsp",
  "schemarefly.trace.server": "messages"
}
```

### Packaging Extension

**Create .vsix file:**
```bash
npm run package
# Creates: schemarefly-0.1.0.vsix
```

**Install locally:**
```bash
code --install-extension schemarefly-0.1.0.vsix
```

**Uninstall:**
```bash
code --uninstall-extension schemarefly.schemarefly
```

---

## Contributing

### Before Submitting Changes

1. **Lint your code:**
   ```bash
   npm run lint
   ```

2. **Compile without errors:**
   ```bash
   npm run compile
   ```

3. **Test all features:**
   - Extension activation
   - LSP server connection
   - Diagnostics
   - Commands
   - Settings

4. **Update documentation:**
   - Update README.md if adding features
   - Update this file if changing development workflow

### Code Style

- Use TypeScript strict mode
- Follow ESLint rules
- Use async/await for asynchronous operations
- Add comments for complex logic
- Keep functions focused and testable

---

## Resources

### VS Code Extension Development

- [VS Code Extension API](https://code.visualstudio.com/api)
- [LSP Specification](https://microsoft.github.io/language-server-protocol/)
- [vscode-languageclient](https://www.npmjs.com/package/vscode-languageclient)

### SchemaRefly

- [SchemaRefly Repository](https://github.com/owner/schemarefly)
- [LSP Server Implementation](../../crates/schemarefly-lsp/)
- [Main README](../../README.md)

### dbt

- [dbt Documentation](https://docs.getdbt.com/)
- [dbt Contracts](https://docs.getdbt.com/docs/collaborate/govern/model-contracts)

---

## Quick Reference

### Common Commands

```bash
# Install dependencies
npm install

# Compile TypeScript
npm run compile

# Watch mode (auto-compile)
npm run watch

# Lint code
npm run lint

# Build LSP server
cargo build --bin schemarefly-lsp

# Package extension
npm run package
```

### Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Launch Extension Development Host | F5 |
| Reload Extension | Cmd+R / Ctrl+R |
| Stop Debugging | Shift+F5 |
| Command Palette | Cmd+Shift+P / Ctrl+Shift+P |
| Open Settings | Cmd+, / Ctrl+, |
| Toggle Output Panel | Cmd+Shift+U / Ctrl+Shift+U |

---

## Support

For issues or questions:
- Check the [Output panel](#view-extension-logs) for error messages
- Review [Common Issues](#common-issues)
- Check LSP server logs
- Open an issue on GitHub

---

**Happy developing! 🚀**
