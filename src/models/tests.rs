#![expect(
    clippy::panic_in_result_fn,
    reason = "Test assertions report regressions"
)]

use async_lsp::lsp_types::{Range, SymbolKind};

use super::{FileAnalysis, ImplicitEdge, SymbolAnalysis, SymbolIndex, SymbolSemantics};
use crate::{Result, graph};

fn symbol(name: &str) -> SymbolAnalysis {
    SymbolAnalysis {
        qualname: name.to_owned(),
        kind: SymbolKind::FUNCTION,
        range: Range::default(),
        selection_range: Range::default(),
        references: None,
        semantics: SymbolSemantics {
            reportable: true,
            top_level: true,
            entrypoint: false,
            execution_range: None,
        },
    }
}

fn edge(from: usize, target: usize) -> ImplicitEdge {
    ImplicitEdge {
        from: SymbolIndex(from),
        target: SymbolIndex(target),
        reason: "test activation",
    }
}

#[test]
fn rejects_out_of_bounds_endpoints() -> Result<()> {
    for (from, target, endpoint) in [(1, 0, "from"), (0, 1, "target"), (usize::MAX, 0, "from")] {
        let error = FileAnalysis::new(
            "/invalid.py".into(),
            vec![symbol("only")],
            vec![edge(from, target)],
        )
        .err()
        .ok_or("out-of-bounds endpoint was accepted")?;
        let expected = format!(
            "invalid implicit edge 0 (test activation) in /invalid.py: {endpoint} symbol index {} is out of bounds for 1 symbols",
            if endpoint == "from" { from } else { target },
        );
        assert_eq!(
            error.to_string(),
            expected,
            "diagnostic must identify the invalid edge"
        );
    }
    Ok(())
}

#[test]
fn empty_files_allow_no_edges() -> Result<()> {
    let empty = FileAnalysis::new("/empty.py".into(), Vec::new(), Vec::new())?;
    assert!(empty.symbols().is_empty(), "empty files have no symbols");
    assert!(
        FileAnalysis::new("/empty.py".into(), Vec::new(), vec![edge(0, 0)]).is_err(),
        "even index zero is invalid in an empty file"
    );
    Ok(())
}

fn sample_files() -> Result<Vec<FileAnalysis>> {
    Ok(vec![
        FileAnalysis::new("/first.py".into(), vec![symbol("other")], vec![edge(0, 0)])?,
        FileAnalysis::new(
            "/second.py".into(),
            vec![symbol("caller"), symbol("callee")],
            vec![edge(0, 1)],
        )?,
    ])
}

#[test]
fn validated_edges_keep_their_file_and_direction() -> Result<()> {
    let files = sample_files()?;
    let built = graph::build(&files);
    let endpoints: Vec<_> = built
        .edges
        .iter()
        .map(|edge| (edge.from.0, edge.target.0))
        .collect();
    // Each module precedes its declarations: [module, other, module, caller, callee].
    assert_eq!(
        endpoints,
        [(1, 1), (3, 4)],
        "edges must retain their file and direction"
    );
    assert!(
        built
            .edges
            .iter()
            .all(|edge| matches!(edge.origin, graph::EdgeOrigin::Implicit("test activation"))),
        "retain activation reasons"
    );
    Ok(())
}

#[test]
fn reference_updates_preserve_declarations_and_edges() -> Result<()> {
    let mut file = sample_files()?.pop().ok_or("missing sample file")?;
    for (_, references) in file.reference_slots_mut() {
        *references = Some(Vec::new());
    }
    let results: Vec<_> = file
        .symbols()
        .iter()
        .map(|symbol| (symbol.qualname.as_str(), symbol.references.is_some()))
        .collect();
    assert_eq!(
        results,
        [("caller", true), ("callee", true)],
        "reference updates preserve declarations"
    );
    assert_eq!(file.implicit_edges().len(), 1, "retain the validated edge");
    Ok(())
}
