# Jett Language — VS Code Extension

Syntax highlighting for the [Jett programming language](https://github.com/your-repo/jett).

## Features

- Syntax highlighting for all Jett keywords, types, operators, strings, numbers, and comments
- String interpolation support (`"hello {name}"`)
- Auto-closing pairs for parentheses, brackets, braces, and quotes
- Comment toggling with `#`
- Indentation rules for colon-terminated blocks

## Installation

### Option 1: Copy to extensions directory

Copy the entire `editor/vscode/` folder to your VS Code extensions directory:

**Linux / macOS:**
```sh
mkdir -p ~/.vscode/extensions/jett-lang
cp -r . ~/.vscode/extensions/jett-lang/
```

**Windows:**
```powershell
Copy-Item -Recurse . "$env:USERPROFILE\.vscode\extensions\jett-lang"
```

Then restart VS Code.

### Option 2: Symbolic link (for development)

Create a symbolic link so changes to the source are reflected immediately:

**Linux / macOS:**
```sh
ln -s "$(pwd)" ~/.vscode/extensions/jett-lang
```

**Windows (run as Administrator):**
```powershell
New-Item -ItemType SymbolicLink -Path "$env:USERPROFILE\.vscode\extensions\jett-lang" -Target (Get-Location)
```

Then restart VS Code or run **Developer: Reload Window**.

### Option 3: Install from VSIX

If a `.vsix` package is available:

```sh
code --install-extension jett-lang-0.1.0.vsix
```

## Supported file extensions

- `.jett`

## Highlighted elements

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
