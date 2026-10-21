<p align="center">
  <picture>
    <source srcset="https://raw.githubusercontent.com/milssky/ghostrefs/master/docs/_static/images/ghostrefs.png" media="(prefers-color-scheme: dark)">
    <img src="https://raw.githubusercontent.com/milssky/ghostrefs/master/docs/_static/images/ghostrefs-transparent.png#gh-light-mode-only" alt="ghostrefs logo" width="320" height="auto" />
  </picture>
</p>

<h1 align="center">ghostrefs</h1>

<p align="center">
  <em>Find code kept alive only by tests.</em>
</p>

## Features

- [x] Finds production symbols whose only known consumers are tests
- [x] Follows transitive dependencies, including `test → helper → target`
- [x] Uses language server references to build a usage graph
- [x] Explains findings with reference locations and test chains
- [x] Supports custom production entry points and ignore patterns
- [x] Written in Rust, currently supports Python via Pyright

## Installation

Requires Rust with Cargo and `pyright-langserver` in your `PATH`.

Install from [crates.io](https://crates.io/crates/ghostrefs):

```bash
npm install -g pyright
cargo install ghostrefs
```

## Example

From a checkout of this repository, run against the included sample project:

```bash
ghostrefs --root . --source fixtures/python/project --tests fixtures/python/tests
```

```text
TestOnly …/main.py:6:5 test_only_used_func (Function)
TestOnly …/main.py:14:7 TestOnlyUsedClass (Class)
TestOnly candidates: 2 (6 production symbols analyzed)
```

For your own project, set `--root` to the directory matching your Python imports.
The `--source` and `--tests` paths are relative to that root.

Add `--verbose` to see why a symbol was flagged, or `--graph` to inspect the usage graph.
Configure ignore patterns, production entry points, and Pyright settings in
`ghostrefs-lsp.json` at the project root.

Findings are candidates for review: dynamic imports, reflection, and external
consumers may be invisible to the language server.

## Documentation

Start with [Getting started](docs/index.rst), then explore the
[CLI reference](docs/pages/cli.rst), [configuration](docs/pages/configuration.rst),
and [analysis model](docs/pages/analysis.rst).

See the [development guide](docs/pages/contributing.rst) to build the searchable
HTML documentation locally.
