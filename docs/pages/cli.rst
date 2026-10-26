Command-line reference
======================

.. code-block:: text

   ghostrefs --root <ROOT> --source <SOURCE> --tests <TESTS>... [--verbose] [--graph | --fail-on-findings]

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
     - Required test files, directories, or glob patterns. Accepts multiple values and repeated flags. Relative to the workspace root.

Explicit absolute file and directory paths are also accepted. Selected paths
are canonicalized before use. A selected directory includes its entire subtree.
Tests may live inside the source directory; files under tests are excluded
from production findings. The source directory cannot be inside or equal to
any selected test directory.

The scan collects ``.py`` files throughout the workspace, including consumers
outside ``--source``. Source directories and selected test files or directories
outside the workspace are also collected. For predictable import resolution,
prefer a common workspace root containing both.

Directory traversal skips ``.git``, ``.venv``, ``venv``, ``__pycache__``, and ``target``,
and does not descend into symlinked directories. ``.gitignore`` is not used
as a scan filter. See :doc:`configuration` for analysis ignore rules.

Test selection
--------------

.. code-block:: bash

   ghostrefs --root . --source app --tests tests/unit tests/integration
   ghostrefs --root . --source app --tests tests/unit --tests app/test_services.py
   ghostrefs --root . --source app --tests '**/test_*.py' --tests '**/tests/**'

Values are combined as a union; duplicates and overlapping selections do not
cause files to be collected more than once. Files not selected as tests keep
their normal production or outside-scope classification.

Quote glob patterns to prevent shell expansion. Patterns are relative to
``--root`` and cannot be absolute or contain a ``..`` component. ``*`` matches
within one path component, ``?`` matches one character, and ``**`` supports
recursive matching. For example, ``test*`` selects matching entries directly
under the root, while ``**/test_*.py`` selects test files at any depth.
Character classes and brace alternatives are also supported. Glob expansion
uses the traversal exclusions listed above. An existing literal path takes
precedence over glob syntax, including special characters in its name.

Every explicit path must exist, every glob must match at least one file or
directory, and malformed patterns are errors. These checks happen before LSP
collection. Matching an empty directory is allowed, as with an explicit path.
Selection classifies entire files; it does not identify test functions inside
files containing production code.

The sample under ``fixtures/python/scattered`` contains two separate test
directories and a test file inside the production directory. These commands
both report ``unit_only``, ``integration_only``, and ``colocated_only``:

.. code-block:: bash

   ghostrefs --root fixtures/python/scattered --source app \
     --tests tests/unit checks/integration --tests app/test_services.py
   ghostrefs --root fixtures/python/scattered --source app --tests '**/test_*.py'

Output options
--------------

.. list-table::
   :header-rows: 1
   :widths: 24 76

   * - Option
     - Meaning
   * - ``-v, --verbose``
     - Add roots, references, and test chains to the normal report on stdout.
   * - ``--graph``
     - Print usage graph edges in place of the findings report.
   * - ``--fail-on-findings``
     - Exit with status ``1`` when TestOnly candidates are found. Cannot be combined with ``--graph``.
   * - ``-h, --help``
     - Show usage and options.
   * - ``-V, --version``
     - Show the installed version.

Collection progress is always printed to stderr, including in graph mode.

Normal report
-------------

Only ``TestOnly`` candidates are printed. The final count of analyzed symbols
includes other usage statuses; it is not a count of findings.

.. code-block:: text

   /workspace/app/services.py:6:5 legacy_helper (Function)
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

Graph mode prints collection progress to stderr. ``--verbose`` does not add
roots, test chains, or the findings summary in graph mode.
Graph mode builds the graph without running entry-point resolution or usage
classification.

Exit behavior
-------------

A completed scan exits with status ``0`` by default, including when candidates
are found. With ``--fail-on-findings``, it exits with status ``1`` if at least
one TestOnly candidate is reported, or ``0`` if none are found. The report is
printed normally in either case, including with ``--verbose``.

.. code-block:: bash

   ghostrefs --root . --source app --tests tests --fail-on-findings

Errors produce a nonzero status regardless of this flag. Graph mode does not
classify findings, so ``--graph`` and ``--fail-on-findings`` are mutually exclusive.

Output is human-readable text. There is no JSON output.
