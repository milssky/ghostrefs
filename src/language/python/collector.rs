use crate::models::{
    Position, Reference, ReferenceKind, Scope, ScopeKind, SourceSpan, Symbol, SymbolKind,
};

struct ParseContext<'a> {
    source: &'a str,
    scope: Vec<Scope>,
    symbols: Vec<Symbol>,
    references: Vec<Reference>,
}

fn source_span(node: tree_sitter::Node<'_>) -> SourceSpan {
    let range = node.range();
    SourceSpan {
        start_byte: range.start_byte,
        end_byte: range.end_byte,
        start: Position {
            row: range.start_point.row,
            column: range.start_point.column,
        },
        end: Position {
            row: range.end_point.row,
            column: range.end_point.column,
        },
    }
}

fn collect_definition(
    node: tree_sitter::Node<'_>,
    ctx: &mut ParseContext<'_>,
    scope_kind: ScopeKind,
) {
    let name_node = node
        .child_by_field_name("name")
        .expect("definition must have a name");

    let name = &ctx.source[name_node.byte_range()];

    let qualname = ctx
        .scope
        .iter()
        .map(|scope| scope.name.as_str())
        .chain(std::iter::once(name))
        .collect::<Vec<_>>()
        .join(".");

    let kind = match scope_kind {
        ScopeKind::Class => SymbolKind::Class,
        ScopeKind::Function
            if ctx
                .scope
                .last()
                .is_some_and(|parent| parent.kind == ScopeKind::Class) =>
        {
            SymbolKind::Method
        }
        ScopeKind::Function => SymbolKind::Function,
    };

    ctx.symbols.push(Symbol {
        name: name.to_owned(),
        qualname,
        kind,
        span: source_span(name_node),
    });

    ctx.scope.push(Scope {
        name: name.to_owned(),
        kind: scope_kind,
    });

    // TODO: Needs to collect definitions if signatures, ie
    // def foo(x=bar()):...
    if let Some(body) = node.child_by_field_name("body") {
        collect_items(body, ctx);
    }

    ctx.scope.pop();
}

fn collect_children(node: tree_sitter::Node<'_>, ctx: &mut ParseContext<'_>) {
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        collect_items(child, ctx);
    }
}

fn collect_call(node: tree_sitter::Node<'_>, ctx: &mut ParseContext<'_>) {
    let Some(function) = node.child_by_field_name("function") else {
        return;
    };

    let name = &ctx.source[function.byte_range()];

    ctx.references.push(Reference {
        name: name.to_owned(),
        kind: ReferenceKind::Call,
        span: source_span(function),
    });
}

fn collect_imports(node: tree_sitter::Node<'_>, ctx: &mut ParseContext<'_>) {
    // TODO: Must we collect import with redefinitions and reexport with `as`?
    let mut cursor = node.walk();
    for child in node.children_by_field_name("name", &mut cursor) {
        let name_node = if child.kind() == "aliased_import" {
            child.child_by_field_name("name").unwrap()
        } else {
            child
        };

        ctx.references.push(Reference {
            name: ctx.source[name_node.byte_range()].to_owned(),
            kind: ReferenceKind::Import,
            span: source_span(name_node),
        });
    }
}

fn collect_items(node: tree_sitter::Node<'_>, ctx: &mut ParseContext<'_>) {
    match node.kind() {
        "class_definition" => {
            collect_definition(node, ctx, ScopeKind::Class);
        }
        "function_definition" => {
            collect_definition(node, ctx, ScopeKind::Function);
        }
        "call" => {
            collect_call(node, ctx);
            collect_children(node, ctx);
        }
        "import_statement" | "import_from_statement" => {
            collect_imports(node, ctx);
        }
        _ => {
            collect_children(node, ctx);
        }
    };
}

pub(super) fn collect(root: tree_sitter::Node<'_>, source: &str) -> (Vec<Symbol>, Vec<Reference>) {
    let mut ctx = ParseContext {
        source,
        scope: Vec::new(),
        symbols: Vec::new(),
        references: Vec::new(),
    };

    collect_items(root, &mut ctx);
    (ctx.symbols, ctx.references)
}
