<p align="center">
  <picture>
    <source srcset="https://raw.githubusercontent.com/milssky/ghostrefs/master/docs/_static/images/ghostrefs-transparent.png" media="(prefers-color-scheme: dark)">
    <img src="https://raw.githubusercontent.com/milssky/ghostrefs/master/docs/_static/images/ghostrefs-light.png" alt="ghostrefs logo" width="320" height="auto" />
  </picture>
</p>

<h1 align="center">ghostrefs</h1>

<p align="center">
  <em>Find code kept alive only by tests.</em>
</p>

**A must-have for your coding agent's toolkit.** AI refactors can leave old
implementations behind, kept alive by their own tests. Everything stays green.
The leftovers stay hidden. Run `ghostrefs` after agent-driven changes to expose
test-only code, follow the reference chains, and give your agent concrete
candidates to review and remove.

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
…/main.py:6:5 test_only_used_func (Function)
…/main.py:14:7 TestOnlyUsedClass (Class)
TestOnly candidates: 2 (6 production symbols analyzed)
```

For your own project, set `--root` to the directory matching your Python imports.
The `--source` and `--tests` paths are relative to that root.
`--tests` accepts one or more files, directories, or glob patterns. You can
also repeat the flag. Quote patterns so ghostrefs expands them relative to
`--root`, independently of your shell:

```bash
ghostrefs --root . --source src --tests tests/unit tests/integration
ghostrefs --root . --source app --tests '**/test_*.py' --tests '**/tests/**'
```

A directory includes its entire subtree. Each path must exist and each glob
must match at least one file or directory. Patterns classify whole files as
tests; they do not select test functions within production files.

Try the scattered-tests fixture with either explicit paths or a glob. Both
commands report `unit_only`, `integration_only`, and `colocated_only`:

```bash
ghostrefs --root fixtures/python/scattered --source app \
  --tests tests/unit checks/integration --tests app/test_services.py
ghostrefs --root fixtures/python/scattered --source app --tests '**/test_*.py'
```


Collection progress is always printed to stderr. Add `--verbose` to see why a
symbol was flagged, or `--graph` to inspect the usage graph.
Add `--fail-on-findings` to exit with status `1` when TestOnly candidates are
found, for example in CI. A scan with no findings exits with status `0`.
This flag cannot be combined with `--graph`.
Configure ignore patterns, production entry points, and Pyright settings in
`ghostrefs-lsp.json` at the project root.

Ghostrefs uses a single Pyright process and sends reference requests sequentially.

Findings are candidates for review: dynamic imports, reflection, and external
consumers may be invisible to the language server.

## GitHub Action

Add `.github/workflows/ghostrefs.yml` to your Python project:

```yaml
name: ghostrefs
on: [push, pull_request]

permissions:
  contents: read

jobs:
  ghostrefs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false
      - uses: milssky/ghostrefs@master
        with:
          source: app
          tests: tests
```

The action installs Pyright and builds ghostrefs from the selected action ref.
Use a runner with Rust/Cargo and Node.js/npm installed, such as `ubuntu-latest`.
Set up your project's Python dependencies before the action if needed.

`root` defaults to `.`; `source` and `tests` are relative to it. For multiple
test paths or globs, pass one per line using `tests: |`.
Findings fail the step by default; set `fail-on-findings: 'false'` to only report
them. Pin the action to a commit SHA for reproducible workflows.
See the [GitHub Action guide](docs/pages/github-action.rst) for inputs and examples.

## Documentation

Start with [Getting started](docs/index.rst), then explore the
[CLI reference](docs/pages/cli.rst), [configuration](docs/pages/configuration.rst),
and [analysis model](docs/pages/analysis.rst).

See the [development guide](docs/pages/contributing.rst) to build the searchable
HTML documentation locally.
