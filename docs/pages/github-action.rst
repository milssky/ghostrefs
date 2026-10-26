GitHub Action
=============

Add ``.github/workflows/ghostrefs.yml`` to your Python project:

.. code-block:: yaml

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
             tests: |
               tests/unit
               **/test_*.py

The action installs Pyright and builds ghostrefs from the selected action ref.
Use a runner with Rust/Cargo and Node.js/npm installed, such as ``ubuntu-latest``.
Prepare your project's Python dependencies before the action if needed for
import resolution. Pin the action to a commit SHA for reproducible workflows.

Inputs
------

* ``root``: workspace root; defaults to ``.``.
* ``source``: required production directory, relative to ``root``.
* ``tests``: required test paths or glob patterns, relative to ``root``, one per
  line. Do not add shell quotes inside a multiline value.
* ``fail-on-findings``: defaults to ``'true'``. Set to ``'false'`` to report
  findings without failing the step. Analysis errors still fail the step.

Findings appear in the workflow logs. The action reads ``ghostrefs-lsp.json``
from ``root``; see :doc:`configuration` for ignore patterns and entry points,
and :doc:`cli` for test selection rules.
