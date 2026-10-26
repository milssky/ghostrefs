Getting started
===============

.. important::

   **Python only for now.** ``ghostrefs`` currently supports Python projects via
   Pyright. Other programming languages are not supported yet but we have infrastructure
   code for it.

``ghostrefs`` finds production code whose only known consumers are tests.
It asks a language server for references, follows dependencies through a usage
graph, and reports candidates for review. The current release analyzes Python
projects using Pyright.

**A must-have for your coding agent's toolkit.** AI refactors can leave old
implementations behind, kept alive by their own tests. Everything stays green.
The leftovers stay hidden. Run ``ghostrefs`` after agent-driven changes to expose
test-only code, follow the reference chains, and give your agent concrete
candidates to review and remove.

Installation
------------

You need Rust with Cargo, and Node.js with npm. Install
`Rust <https://www.rust-lang.org/tools/install>`_ and
`Node.js <https://nodejs.org/en/download>`_ if they are not already available.

.. tabs::

   .. tab:: Published release

      .. code-block:: bash

         npm install -g pyright
         cargo install ghostrefs --locked

   .. tab:: From source

      .. code-block:: bash

         git clone https://github.com/milssky/ghostrefs.git
         cd ghostrefs
         npm install -g pyright
         cargo install --path . --locked

Check that both commands are available in the shell where you will run the scan:

.. code-block:: bash

   ghostrefs --version
   command -v pyright-langserver
   pyright --version

Your first scan
---------------

Suppose your Python project has this layout:

.. code-block:: text

   my-project/
   ├── app/
   │   └── services.py
   └── tests/
       └── test_services.py

Run from ``my-project``:

.. code-block:: bash

   ghostrefs --root . --source app --tests tests

``--root`` is the workspace directory used to resolve Python imports.
``--source`` selects the production directory to report on, and ``--tests``
identifies test consumers using one or more files, directories, or quoted
glob patterns. Values are resolved relative to ``--root``. Explicit paths
must exist and every glob must match. See :doc:`pages/cli` for examples.

Try the sample project
----------------------

From a checkout of the ghostrefs repository:

.. code-block:: bash

   ghostrefs --root . --source fixtures/python/project --tests fixtures/python/tests

Example output (the absolute path prefix is shortened here):

.. code-block:: text

   …/main.py:6:5 test_only_used_func (Function)
   …/main.py:14:7 TestOnlyUsedClass (Class)
   TestOnly candidates: 2 (6 production symbols analyzed)

Each finding gives the file, line, column, qualified symbol name, and symbol
kind. Locations in the report start at 1.

Understand a finding
--------------------

Add ``--verbose`` to inspect reference locations, inferred production roots,
and a chain leading from a test to each candidate:

.. code-block:: bash

   ghostrefs --root . --source app --tests tests --verbose

.. important::

   A ``TestOnly`` finding means that the collected graph has a path from tests
   to the symbol, with no known production, outside-scope, or unknown consumer
   path. Review dynamic imports, framework hooks, and external callers before
   removing code. See :doc:`pages/analysis` for the limits of this analysis.

Next steps
----------

* :doc:`pages/cli` describes every command-line option and the output formats.
* :doc:`pages/configuration` covers ignore patterns, production entry points,
  and Pyright settings.
* :doc:`pages/troubleshooting` helps with imports, missing findings, and errors.

Every page is also available as Markdown: replace ``.html`` with ``.md`` in its
URL, or use the Markdown controls beside the page title. ``llms.txt`` indexes
the guide and ``llms-full.txt`` contains it in one file.

.. toctree::
   :hidden:
   :caption: User guide

   self
   pages/cli
   pages/configuration
   pages/analysis
   pages/troubleshooting

.. toctree::
   :hidden:
   :caption: Development

   pages/contributing
