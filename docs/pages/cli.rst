Command-line reference
======================

.. code-block:: text

   ghostrefs --root <ROOT> --source <SOURCE> --tests <TESTS> [--verbose] [--graph]

Paths and scope
---------------

.. list-table::
   :header-rows: 1
   :widths: 24 76

   * - Option
     - Meaning
   * - ``-r, --root <ROOT>``
     - Required workspace root. Relative to the current working directory.
   * - ``-s, --source <SOURCE>``
     - Required production directory. Relative to the workspace root.
   * - ``-t, --tests <TESTS>``
     - Required test directory. Relative to the workspace root.

Absolute paths are also accepted. Directories are canonicalized before use.
Tests may live inside the source directory; files under tests are excluded
from production findings. The source directory cannot be inside or equal to
the test directory.

The scan collects ``.py`` files throughout the workspace, including consumers
outside ``--source``. Source and test directories outside the workspace are
also collected when supplied as absolute paths. For predictable import
resolution, prefer a common workspace root containing both.

Directory traversal skips ``.git``, ``.venv``, ``venv``, ``__pycache__``, and ``target``,
and does not descend into symlinked directories. ``.gitignore`` is not used
as a scan filter. See :doc:`configuration` for analysis ignore rules.

Output options
--------------

.. list-table::
   :header-rows: 1
   :widths: 24 76

   * - Option
     - Meaning
   * - ``-v, --verbose``
     - Print collection progress to stderr; add roots, references, and test
       chains to the normal report on stdout.
   * - ``--graph``
     - Print usage graph edges in place of the findings report.
   * - ``-h, --help``
     - Show usage and options.
   * - ``-V, --version``
     - Show the installed version.

Normal report
-------------

Only ``TestOnly`` candidates are printed. The final count of analyzed symbols
includes other usage statuses; it is not a count of findings.

.. code-block:: text

   TestOnly /workspace/app/services.py:6:5 legacy_helper (Function)
   TestOnly candidates: 1 (12 production symbols analyzed)

The report includes eligible production declarations such as top-level
variables, functions, classes, and nested scopes. Local variables and parameters
remain in the graph but are omitted from the report.

Verbose report
--------------

.. code-block:: bash

   ghostrefs -r . -s app -t tests --verbose

``Production root:`` and ``Unknown root:`` lines describe analysis roots.
Each candidate includes its LSP range, collected references, and one
``test chain:`` showing how a test reaches it. Raw LSP ranges and reference
positions use zero-based coordinates; human-readable symbol locations use
one-based coordinates.

Usage graph
-----------

.. code-block:: bash

   ghostrefs -r . -s app -t tests --graph

An edge points from the consumer to the symbol it uses. For example, a graph
can contain this edge (paths and positions depend on your project):

.. code-block:: text

   /workspace/tests/test_services.py:4:1::test_helper -> /workspace/app/services.py:6:5::legacy_helper [reference 5:12]

``[reference line:column]`` identifies the reference in the consumer.
``[class member]`` marks a conservative edge from a class to one of its
members. ``::<module>`` identifies module-level code and ``::<unresolved>``
means a reference could not be assigned to a collected owner.

When combined with ``--verbose``, graph mode still prints collection progress
to stderr, but does not print roots, test chains, or the findings summary.
Graph mode builds the graph without running entry-point resolution or usage
classification.

Exit behavior
-------------

A completed scan exits with status ``0``, including when candidates are found.
Errors produce a nonzero status. Findings currently do not fail a CI job.

Output is human-readable text. There is no JSON output or dedicated
fail-on-findings option in the current release.
