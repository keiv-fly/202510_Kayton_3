use insta::assert_snapshot;
use kayton_front::tests_support::parse_str;

#[test]
fn parse_simple_function() {
    let source = r#"fn add(a, b):
    let total = a + b
    total
"#;
    let output = parse_str("add.ktn", source);
    assert_snapshot!(
        "simple_function",
        format!("{:#?}\n---\n{:#?}", output.module, output.diagnostics)
    );
}

#[test]
fn parse_if_else_suite() {
    let source = r#"fn classify(x):
    if x > 0:
        "positive"
    elif x < 0:
        "negative"
    else:
        "zero"
"#;
    let output = parse_str("classify.ktn", source);
    assert_snapshot!(
        "if_else_suite",
        format!("{:#?}\n---\n{:#?}", output.module, output.diagnostics)
    );
}

#[test]
fn parse_inline_if_expression() {
    let source = r#"fn absolute(x):
    let sign = if x < 0: -1 else: 1
    if sign == -1:
        -x
    else:
        x
"#;
    let output = parse_str("absolute.ktn", source);
    assert_snapshot!(
        "inline_if",
        format!("{:#?}\n---\n{:#?}", output.module, output.diagnostics)
    );
}

#[test]
fn parse_while_loop() {
    let source = r#"fn countdown(n):
    let current = n
    while current > 0:
        let _ = current
        let current = current - 1
        ()
    ()
"#;
    let output = parse_str("countdown.ktn", source);
    assert_snapshot!(
        "while_loop",
        format!("{:#?}\n---\n{:#?}", output.module, output.diagnostics)
    );
}
