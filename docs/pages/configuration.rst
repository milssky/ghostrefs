Configuration
=============

Create ``ghostrefs-lsp.json`` directly under ``--root``. It is optional.
Project settings override bundled defaults: JSON objects merge recursively;
arrays and scalar values replace their defaults.

Default settings
----------------

.. literalinclude:: ../../ghostrefs-lsp.json
   :language: json

The ``ghostrefs`` object configures analysis. The remaining settings are sent
to Pyright through ``workspace/didChangeConfiguration``. The file must contain
a JSON object; comments and trailing commas are not accepted.

Ignore patterns
---------------

.. code-block:: json

   {
     "ghostrefs": {
       "ignore": ["app/generated/**", "app/migrations/**"]
     }
   }

Patterns match file paths relative to ``--root``. Use ``/`` as the separator.
``*`` matches within a path component, and ``**`` matches across directories.
Patterns must be nonempty, relative, and contain no ``..`` path component.
To ignore a directory's contents, use ``app/generated/**`` rather than
``app/generated``.

Ignored files are still collected and kept in the usage graph. Their symbols
are omitted from findings, and their nodes become unknown roots. A symbol
reachable from ignored code is therefore protected from a ``TestOnly``
classification. Ignore rules are analysis rules, not a way to speed up
file collection.

Production entry points
-----------------------

Register code that production invokes through framework registration,
reflection, or another mechanism the language server cannot follow:

.. code-block:: json

   {
     "ghostrefs": {
       "production_entrypoints": [
         {"path": "app/jobs.py", "qualname": "refresh_cache"},
         {"path": "app/handlers.py", "qualname": "EventHandler.handle"}
       ]
     }
   }

Each entry has exactly two fields:

.. list-table::
   :header-rows: 1
   :widths: 22 78

   * - Field
     - Meaning
   * - ``path``
     - Nonempty relative path to an existing file inside the workspace root
       and production source, outside tests. The file must not be ignored.
   * - ``qualname``
     - Exact dotted declaration name reported by Pyright, for example
       ``refresh_cache`` or ``EventHandler.handle``. Module names are omitted.

An entry point and symbols reachable from it are treated as production-used.
An unresolved or ambiguous name fails analysis rather than silently ignoring
the entry. Duplicate declarations with the same qualified name can be
ambiguous. Use ``--graph`` to inspect the collected names.

The only supported keys inside ``ghostrefs`` are ``ignore`` and
``production_entrypoints``. Unknown keys and unknown entry-point fields are
errors, so misspelled settings do not silently pass.

Pyright settings
----------------

For a ``src`` layout, you may need to add an import search path:

.. code-block:: json

   {
     "python": {
       "analysis": {
         "extraPaths": ["./src"],
         "exclude": ["**/.venv", "**/venv", "**/generated"]
       }
     }
   }

Here ``exclude`` replaces the bundled list, so retain its entries if you need
them. Pyright's ``exclude`` setting controls its analysis configuration;
it does not change ghostrefs' directory traversal or substitute for
``ghostrefs.ignore``. Ghostrefs explicitly opens collected Python files.

For Python import and environment configuration, consult the
`Pyright configuration reference
<https://github.com/microsoft/pyright/blob/main/docs/configuration.md>`_.
The language server executable is currently fixed to
``pyright-langserver --stdio``; it cannot be changed through this file.
