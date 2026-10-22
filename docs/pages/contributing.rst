Development
===========

Run from a checkout with Rust and Pyright installed:

.. code-block:: bash

   cargo run --locked -- --root . --source fixtures/python/project --tests fixtures/python/tests
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets --all-features -- -D warnings

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

Hosting
-------

``.readthedocs.yml`` defines the hosted build. Connect the repository to
Read the Docs to enable hosting. Set ``READTHEDOCS_CANONICAL_URL`` when building
for another host so the generated Markdown index uses the correct public URL.
The fallback URL assumes a Read the Docs project named ``ghostrefs``.

The documentation CI job builds the HTML with warnings treated as errors and
uploads the result as an artifact. Connecting hosting is a separate step;
the CI build itself does not publish the site.
