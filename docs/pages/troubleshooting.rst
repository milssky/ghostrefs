Troubleshooting
===============

Pyright cannot start
--------------------

Ghostrefs starts ``pyright-langserver --stdio`` from the workspace root.
The executable must be available in the process's ``PATH``:

.. code-block:: bash

   npm install -g pyright
   command -v pyright-langserver
   pyright --version

If this works in a terminal but fails in CI or an editor task, check the
``PATH`` in that environment. Errors such as ``No such file or directory``
can indicate that the language server executable is missing.

Invalid directory or no production files
----------------------------------------

``--source`` and ``--tests`` are relative to ``--root``, not necessarily the
current working directory. All paths must identify existing directories.
At least one supported production ``.py`` file must exist outside tests.

.. code-block:: bash

   ghostrefs --root /workspace/my-project --source app --tests tests

If tests are nested under ``app/tests``, use ``--source app --tests app/tests``.
Source cannot be inside the test directory.

Missing or unexpected findings
------------------------------

First inspect the evidence:

.. code-block:: bash

   ghostrefs -r . -s app -t tests --verbose
   ghostrefs -r . -s app -t tests --graph

Check the following:

* Is the symbol under ``--source`` and outside ``--tests``?
* Does the selected workspace root match the project's Python imports?
* Are dependencies and import paths configured for Pyright?
* Does a production root, outside-scope consumer, ignored file, or unknown
  root reach the symbol?
* Is a framework invoking the symbol dynamically? Register it as a production
  entry point if the static references do not capture that caller.

Only ``TestOnly`` symbols are printed. Unreferenced and unreachable symbols
are omitted. See :doc:`analysis` for classification rules and limitations.

Directory traversal skips ``target`` so packaged copies of the fixture tests
under ``target/package`` cannot become outside-scope consumers of the original
fixture symbols. If you see this behavior with an older local binary, rebuild
it with ``cargo build --locked`` and rerun the scan.

Entry point not found or ambiguous
----------------------------------

The configured ``path`` must identify a collected production file inside the
workspace root, and ``qualname`` must exactly match its symbol name.
Use ``Worker.run`` for a method, without the package or module prefix.
Inspect ``--graph`` output to check the collected qualified names.

Multiple declarations with the same qualified name in a file are ambiguous.
Ghostrefs reports their candidate locations. Resolve the ambiguity in the
configuration or source before running normal analysis again.

Ignore rules do not reduce scan time
------------------------------------

``ghostrefs.ignore`` excludes findings and preserves conservative dependency
paths. It does not skip file collection or LSP requests. Use a focused
workspace root while retaining the production and test consumers you need
to analyze. See :doc:`configuration` for the difference between analysis
ignore rules and Pyright settings.

Large workspace scans are slow
-----------------------------

Use ``--verbose`` to see the number of collected symbols, reference requests,
and elapsed time while requests are processed. Reference searches in Pyright
usually dominate scan time; rebuilding Ghostrefs in release mode alone may
have little effect on that work.

Normal reports follow references backwards from production declarations and
skip searches for declarations with no path to a reportable symbol. This
includes nested variables and attributes, which cannot own dependencies.
``--graph`` collects every declaration's references and can take longer.
Both modes retain all workspace files so outside-scope consumers remain visible.

CI succeeds when candidates are found
-------------------------------------

This is the current behavior: findings are review candidates and a successful
scan returns ``0``. There is no built-in fail-on-findings flag. Save the report
as a CI artifact if you want to review it after a job:

.. code-block:: bash

   ghostrefs -r . -s app -t tests > ghostrefs-report.txt

Reporting an issue
------------------

Include the ghostrefs version, Pyright version, command, workspace layout,
relevant configuration, and a minimal Python example. Add verbose or graph
output when it helps explain the unexpected dependency path; review it for
private paths and code before sharing.

Open a report in the `issue tracker
<https://github.com/milssky/ghostrefs/issues>`_.
