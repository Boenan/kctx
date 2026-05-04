# kctx

`kctx` is a interactive Kubernetes context and namespace switcher built in Rust.

## Features

- **Interactive Context Switching:** Switch between Kubernetes contexts with fuzzy search.
- **Namespace Management:** Switch namespaces within the current context, featuring cluster-aware completion.
- **Context Deletion:** Remove context entries from your kubeconfig.
- **Quick Info:** Instantly view your current context and namespace.
- **Shell Completions:** Native support for various shells.

## Installation

### From Source

Ensure you have [Rust and Cargo](https://rustup.rs/) installed, then run:

```bash
cargo install --path .
```

or download the binary in Github releases page.

## Usage

### Switch Context

To switch the active context interactively:

```bash
kctx context
```

To switch to a specific context:

```bash
kctx context <context-name>
```

### Switch Namespace

To switch the active namespace interactively:

```bash
kctx namespace
```

To switch to a specific namespace:

```bash
kctx namespace <namespace-name>
```

### View Current Info

Show the active context and namespace:

```bash
kctx info
```

### Delete a Context

Interactively delete a context:

```bash
kctx delete
```

Or specify the name:

```bash
kctx delete <context-name>
```

## Shell completion

Support completion for `bash`, `elvish`, `fish`, `powershell`, `zsh`. Put the output in your shell config file.

```bash
kctx completion <shell>
```

### Example for `fish`:

```bash
kctx completion fish > ~/.config/fish/completions/kctx.fish
```
