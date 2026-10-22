How analysis works
==================

Ghostrefs looks for code kept alive only by tests. A direct reference count
cannot detect the full pattern: a test can call a helper that calls another
production symbol. Ghostrefs follows the complete collected chain.

.. code-block:: text

   test_checkout → legacy_helper → legacy_formatter

Both production symbols can be candidates if there is no production,
outside-scope, or unknown path to them. If a production entry point reaches
the helper, that path also protects the formatter.

From references to a graph
--------------------------

1. Collect Python files from the workspace and requested source/test paths.
2. Start a single Pyright process, apply settings, and open all collected files.
3. Collect hierarchical document symbols, then request references sequentially
   from the same process.
   Normal reports start with reportable production declarations and
   follow incoming references to discover consumers, including class activation,
   until no new consumers appear. Declarations with no path to a reported
   symbol need no reference search. ``--graph`` requests every declaration.
4. Assign references to their owning scope or module. Add edges from each
   consumer to the referenced symbol.
5. Walk the graph from production, test, outside-scope, and unknown roots.

References are static usage information, not a runtime execution trace.
Imports and module-level expressions can also contribute edges.
Pyright diagnostics are not requested; type information needed for symbols and
references is evaluated on demand.

Roots and conservative behavior
--------------------------------

Production module nodes are production roots. Top-level declarations named
``main`` and top-level declarations with reference results but no known
consumer are also inferred as production roots. This protects their
dependencies when a real application entry point is not explicit in the
graph. Configured :doc:`production entry points <configuration>` add roots.

Test modules and symbols under ``--tests`` are test roots. Collected consumers
outside both source and tests are outside-scope roots. Missing reference
results, unresolved owners inside production source, and ignored nodes can
introduce unknown roots.

Class activation conservatively reaches its members. This can protect members
that are not individually called, favoring fewer false positives.

Usage statuses
--------------

The analyzer classifies eligible production symbols using the following
statuses. The normal CLI report prints only ``TestOnly``.

.. list-table::
   :header-rows: 1
   :widths: 26 74

   * - Status
     - Meaning
   * - ``TestOnly``
     - Reachable from tests, with no known production, outside-scope, or
       unknown path.
   * - ``UsedInProduction``
     - Reachable from production roots, including configured entry points.
   * - ``UsedOutsideScope``
     - Reachable from an outside-scope consumer, without a production path.
   * - ``Unknown``
     - Insufficient reference information, or reachable through unknown
       consumers without a stronger known production/outside-scope path.
   * - ``Unreferenced``
     - No known non-self consumer, subject to entry-point and reachability
       rules. Omitted from the candidate report.
   * - ``Unreachable``
     - Has a consumer but no path from any of the analysis roots.

Missing reference results take precedence unless the declaration is an
explicit production entry point. A known production path takes precedence
over outside-scope, unknown, and test paths. Outside-scope and unknown paths
also prevent a ``TestOnly`` finding.

Limits
------

* The current language support is Python via Pyright; other file extensions
  are not collected.
* Dynamic imports, reflection, monkey-patching, generated code, and framework
  registries may hide callers. Add known production entry points where needed.
* Consumers outside the scanned workspace may be invisible. A public library
  symbol used only by local tests may still be used by downstream packages.
* Unresolved imports or an incorrect Python environment can leave references
  incomplete. Check the project setup before trusting a scan.
* The conservative roots and class edges can hide some test-only code.
  An empty report is not proof that every declaration is used in production.

Ghostrefs does not run tests or modify source files. Review a candidate's
references and test chain, check its production entry points, then use your
usual tests and review process to validate a removal.
