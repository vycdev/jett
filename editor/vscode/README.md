# Jett Language - VS Code Extension

Syntax highlighting for the [Jett programming language](https://github.com/vycdev/jett).

## Features

- Syntax highlighting for Jett keywords, types, operators, strings, numbers, and comments
- String interpolation support (`"hello {name}"`)
- Richer scopes for custom types, function calls, generic type arguments, parameters, fields, and enum members
- Optional theme-aware Jett file icons for `.jett` files
- Auto-closing pairs for parentheses, brackets, braces, and quotes
- Comment toggling with `#`
- Indentation rules for colon-terminated blocks

## Build a VSIX

From the repository root:

```sh
cd editor/vscode
npm install
npm run package
```

This creates `jett-lang-0.1.2.vsix` in this directory.

If `vsce` is already installed globally, you can also run:

```sh
vsce package
```

## Install

Install the packaged extension with:

```sh
code --install-extension jett-lang-0.1.2.vsix
```

Then reload any open VS Code windows.

To use the bundled file icon, open **File: Preferences: File Icon Theme** from the command palette and select **Jett File Icons**. The icon theme uses the dark logo in dark themes and the light logo in light themes.

## Development Install

For extension development, copy or symlink this folder into your VS Code extensions directory.

**Linux / macOS:**

```sh
mkdir -p ~/.vscode/extensions/jett-lang
cp -r . ~/.vscode/extensions/jett-lang/
```

**Windows:**

```powershell
Copy-Item -Recurse . "$env:USERPROFILE\.vscode\extensions\jett-lang"
```

For a live development symlink on Windows, run PowerShell as Administrator:

```powershell
New-Item -ItemType SymbolicLink -Path "$env:USERPROFILE\.vscode\extensions\jett-lang" -Target (Get-Location)
```

Then restart VS Code or run **Developer: Reload Window**.

## Supported File Extensions

- `.jett`

## Highlighted Elements

| Element | Scope | Example |
|---|---|---|
| Comments | `comment.line.number-sign` | `# this is a comment` |
| Control keywords | `keyword.control` | `function`, `if`, `return`, `for` |
| Other keywords | `keyword.other` | `struct`, `enum`, `actor`, `verify` |
| Built-in types | `storage.type` | `int64`, `string`, `list`, `bool` |
| Capability types | `support.type` | `Stdout`, `Filesystem`, `Network` |
| Boolean literals | `constant.language` | `true`, `false` |
| Number literals | `constant.numeric` | `42`, `3.14` |
| String literals | `string.quoted.double` | `"hello {name}"` |
| Operators | `keyword.operator` | `==`, `+`, `&&`, `!` |
| Function names | `entity.name.function` | `function fibonacci(` |
| Type names | `entity.name.type` | `struct Point:` |
| Namespace names | `entity.name.namespace` | `namespace app` |
| Field access | `variable.other.member` | `.field_name` |
