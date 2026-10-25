Development
===========

Run from a checkout with Rust and Pyright installed:

.. code-block:: bash

   cargo run --locked -- --root . --source fixtures/python/project --tests fixtures/python/tests
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets --all-features -- -D warnings

Code layout
-----------

``src/language.rs`` only declares and exports the language package modules.
``src/language/backend.rs`` defines the ``Language`` contract;
``src/language/registry.rs`` registers implementations and selects a backend
once before collection. Each implementation, such as
``src/language/python.rs``, owns file recognition, directory exclusions,
qualified names, symbol scopes, entry-point conventions, report eligibility,
and implicit activation edges.

``src/lsp.rs`` owns the shared LSP transport and session lifecycle.
``src/lsp/server.rs`` registers server backends; ``src/lsp/pyright.rs`` contains
Pyright's launch configuration and capability workarounds. Languages select
a default server without putting server configuration into the scanner or
collector. The root ``ghostrefs-lsp.json`` is the single source of bundled
settings for all languages and servers. Project configuration overrides those
defaults recursively, independently of server selection.

``src/collect.rs`` coordinates requests and delegates symbol normalization to
the language. ``src/models.rs`` defines the normalized metadata consumed by
``src/graph.rs`` and ``src/analysis.rs``. Keep language rules in the language
module and server-specific behavior in the server backend when extending
support. The current run still requires files of one language and uses one
server session; Python via Pyright remains the only registered combination.

Adding a language
-----------------

To support another language:

1. Create ``src/language/<language>.rs`` with a type implementing the
   ``Language`` trait from ``src/language/backend.rs``. Use ``Python`` in
   ``src/language/python.rs`` as the existing example.
2. Declare the module in ``src/language.rs``.
3. Add a reference to the implementation to ``LANGUAGES`` in
   ``src/language/registry.rs``.

The implementation provides the LSP language ID and default server, recognizes
source files, excludes language-specific directories, validates qualified
names, and converts document symbols into ``FileAnalysis``. Populate
``SymbolSemantics`` with report eligibility, top-level and entry-point flags,
and execution ranges. Construct the result with ``FileAnalysis::new(path,
symbols, implicit_edges)``. Each ``ImplicitEdge`` uses ``SymbolIndex`` values
into that file's final symbol list. The constructor rejects out-of-bounds
endpoints; the symbol order and edges cannot change afterward. Reference
results can still be updated through ``reference_slots_mut``. Graph construction
and usage analysis consume this metadata without selecting a language again.

If the language needs a new LSP server, add its module under ``src/lsp/``,
declare it in ``src/lsp.rs``, and add a variant and definition to ``Server`` in
``src/lsp/server.rs``. Follow ``src/lsp/pyright.rs`` for the executable,
arguments, and server-specific client capabilities. Put bundled settings in
the root ``ghostrefs-lsp.json``; do not create a separate defaults file for
each server.

During scanning, the registry checks whether any implementation recognizes
each file. Before collection, ``detect()`` selects the implementation for the
first collected file and verifies that it recognizes all remaining files.
The collector then passes the selected ``&dyn Language`` to session startup
and symbol normalization. Configuration also uses the registry to validate
each configured entry point's qualified name according to its file's language.

Registering another language enables separate runs for that language.
A workspace containing files of multiple registered languages currently
fails validation. Supporting a multilingual monorepo will also require
grouping files by language and managing multiple LSP sessions.

Building the documentation
--------------------------

The guide uses Sphinx with the Shibuya theme. Python 3.12 or newer is required
to build it; Python tooling is independent of the Rust application.

.. code-block:: bash

   python3 -m venv .venv
   .venv/bin/python -m pip install -r docs/requirements.txt
   .venv/bin/python -m sphinx -n -W --keep-going -b html docs docs/_build/html
   .venv/bin/python -m http.server 8000 --directory docs/_build/html

Open ``http://localhost:8000``. The first page is Getting started; there is
no separate landing page. Search, light/dark themes, code-copy buttons,
page navigation, and Markdown export are provided by the theme and extensions.

Sphinx also creates ``llms.txt``, ``llms-full.txt``, and a Markdown version
beside each HTML page. Treat warnings as errors to catch broken internal
references and unsupported export nodes before publishing.

The documentation version is read from ``Cargo.toml``. Bundled configuration
is included directly from ``ghostrefs-lsp.json`` so defaults stay current.
The transparent brand image is ``docs/_static/images/ghostrefs-transparent.png``.
It is used for both themes and as the favicon. The light-theme logo uses CSS
color inversion, which preserves the PNG's alpha channel.
