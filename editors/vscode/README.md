# SchemaRefly for VS Code

Schema contract verification for dbt - Catch breaking changes before they break production.

## Features

### Real-time Diagnostics

Get instant feedback on schema contract violations as you edit SQL files:

- **Contract violations** - Missing columns, type mismatches, extra columns
- **SQL issues** - Parse errors, unsupported syntax, inference warnings
- **Jinja errors** - Template rendering issues, undefined variables

### Hover Information

Hover over SQL to see the inferred schema:

- Column names and types
- Type inference from expressions
- Multi-dialect support (BigQuery, Snowflake, Postgres)

### Go-to-Definition

Jump from code to definitions:

- `ref('model')` - Jump to model SQL file
- Contract columns - Jump to schema.yml definition

### Offline Mode

Works without warehouse connection:

- Uses `target/manifest.json` from dbt compile
- Schema inference from SQL AST
- No credentials required for basic validation

## Requirements

1. **dbt project** - Extension activates in dbt project directories
2. **Compiled dbt project** - Run `dbt compile` to generate `target/manifest.json`
3. **SchemaRefly LSP** - Language server binary (see Installation)

## Installation

### Option 1: Install from VS Code Marketplace

Search for "SchemaRefly" in the VS Code extensions panel.

### Option 2: Install from VSIX

Download the latest `.vsix` file from [GitHub Releases](https://github.com/owner/schemarefly/releases) and install:

```bash
code --install-extension schemarefly-0.1.0.vsix
```

### Install the Language Server

The extension needs the `schemarefly-lsp` binary:

```bash
# Download from releases
curl -fsSL https://github.com/owner/schemarefly/releases/latest/download/schemarefly-x86_64-unknown-linux-gnu.tar.gz | tar -xz
sudo mv schemarefly-*/schemarefly-lsp /usr/local/bin/

# Or build from source
cargo build --release --bin schemarefly-lsp
cp target/release/schemarefly-lsp /usr/local/bin/
```

## Configuration

### Extension Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `schemarefly.serverPath` | `""` | Path to schemarefly-lsp binary |
| `schemarefly.trace.server` | `"off"` | LSP trace level (`off`, `messages`, `verbose`) |
| `schemarefly.diagnostics.onSave` | `true` | Run diagnostics on file save |
| `schemarefly.diagnostics.onType` | `true` | Run diagnostics as you type |
| `schemarefly.hover.showInferredSchema` | `true` | Show schema on hover |

### Project Configuration

Create `schemarefly.toml` in your dbt project root:

```toml
# SQL dialect
dialect = "bigquery"

# Severity overrides
[severity.overrides]
CONTRACT_EXTRA_COLUMN = "warn"

# Allowlists
[allowlist]
allow_widening = ["staging.*"]
skip_models = ["temp_*"]
```

## Commands

| Command | Description |
|---------|-------------|
| `SchemaRefly: Restart Language Server` | Restart the LSP server |
| `SchemaRefly: Check Contracts` | Run full contract check in terminal |
| `SchemaRefly: Show Output` | Show the extension output channel |

## Usage

1. **Open a dbt project** - Extension activates when `dbt_project.yml` is detected

2. **Compile dbt** - Run `dbt compile` to generate manifest

3. **Edit SQL files** - Diagnostics appear automatically

4. **Hover for schema** - Mouse over SQL to see inferred types

5. **Fix violations** - Update contracts or SQL as needed

## Troubleshooting

### "Language server not found"

Ensure `schemarefly-lsp` is installed and in your PATH, or configure `schemarefly.serverPath`.

### "No manifest.json found"

Run `dbt compile` to generate the manifest file.

### Diagnostics not appearing

1. Check the output panel (`SchemaRefly: Show Output`)
2. Ensure the file is in a dbt project with compiled manifest
3. Try restarting the server (`SchemaRefly: Restart Language Server`)

### Performance issues

For large projects:
- Disable `schemarefly.diagnostics.onType`
- Run checks only on save

## Development

### Building the Extension

```bash
cd editors/vscode
npm install
npm run compile
```

### Running in Development

1. Open the `editors/vscode` folder in VS Code
2. Press F5 to launch Extension Development Host
3. Open a dbt project in the new window

### Packaging

```bash
npm run package
# Creates schemarefly-0.1.0.vsix
```

## Contributing

See the main [SchemaRefly repository](https://github.com/owner/schemarefly) for contribution guidelines.

## License

MIT OR Apache-2.0
