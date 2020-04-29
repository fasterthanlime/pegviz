use std::error::Error;

#[derive(Debug)]
enum State {
    Success,
    Failure,
    Unknown,
}

#[derive(Debug)]
struct Location {
    line: usize,
    column: usize,
    pos: usize,
}

#[derive(Debug)]
struct Node {
    rule: Option<Rule>,
    state: State,
    children: Vec<Node>,
}

#[derive(Debug)]
struct Rule {
    name: String,
    loc: Location,
}

#[derive(Debug)]
enum Line {
    Attempt(Rule),
    Failure,
    Success,
}

peg::parser! {
    grammar tracer() for str {
        pub(crate) rule line() -> Line
            = "[PEG_TRACE] " l:line0() { l }

        rule line0() -> Line
            = r:attempt() { Line::Attempt(r) }
            / fail() { Line::Failure }
            / succ() { Line::Success }

        rule fail()
            = "Failed to match rule " [_]*

        rule succ()
            = "Matched rule " [_]*

        rule attempt() -> Rule
            = "Attempting to match rule " r:node() { r }

        rule node() -> Rule
            = name:$(['A'..='Z' | 'a'..='z' | '0'..='9' | '_']+) " at " line:int() ":" column:int() " (pos " pos:int() ")" {
            Rule {
                name: name.into(),
                loc: Location {
                    line,
                    column,
                    pos,
                }
            }
        }

        rule int() -> usize
            = digits:$(['0'..='9']+) { digits.parse().unwrap() }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);

    let trace_file = args.next().unwrap();
    let source_file = args.next().unwrap();

    let trace_file = std::fs::read_to_string(trace_file)?;
    let source_file = std::fs::read_to_string(source_file)?;

    let mut stack: Vec<Node> = vec![Node {
        rule: None,
        state: State::Success,
        children: vec![],
    }];

    for line in trace_file.lines() {
        let t = match tracer::line(line) {
            Ok(t) => t,
            Err(_) => continue,
        };
        match t {
            Line::Attempt(rule) => {
                let node = Node {
                    rule: Some(rule),
                    state: State::Unknown,
                    children: vec![],
                };
                stack.push(node);
            }
            Line::Success => {
                let mut node = stack.pop().unwrap();
                node.state = State::Success;
                stack.last_mut().unwrap().children.push(node);
            }
            Line::Failure => {
                let mut node = stack.pop().unwrap();
                node.state = State::Failure;
                stack.last_mut().unwrap().children.push(node);
            }
        }
    }

    let root = stack.pop().unwrap();

    println!("<html>");
    println!("<head>");
    println!(
        r#"
        <link href="https://fonts.googleapis.com/css2?family=Open+Sans&family=Ubuntu+Mono:wght@400;700&display=swap" rel="stylesheet">
    "#
    );
    println!("<style>");
    println!(
        "{}",
        r#"
    body {
        font-family: 'Open Sans', sans-serif;
        line-height: 1.4;
    }

    code {
        color: #fefefe;
        font-family: 'Ubuntu Mono', monospace;
    }

    code em, code strong, code span {
        background: #333;
        border-radius: 2px;
        padding: 2px;
    }

    code em {
        font-style: initial;
        color: #676767;
    }

    code strong {
        font-weight: normal;
        background: #3f7d2a;
        padding: 2px;
        color: #fefefe;
    }

    body {
        background: #111;
        color: white;
    }

    details {
        user-select: none;
        cursor: pointer;
        padding-left: 25px;
    }
    *:focus {
        outline: none;
    }

    span.success {
        color: #fefefe;
    }
    span.failure {
        color: #777;
    }
    "#
    );
    println!("</style>");
    println!("</head>");
    println!("<body>");
    for child in &root.children {
        visit(child, None, &source_file);
    }
    println!("</body>");
    println!("</html>");

    Ok(())
}

fn visit(node: &Node, next: Option<&Node>, source_file: &str) {
    let rule = match node.rule.as_ref() {
        Some(rule) => rule,
        None => return,
    };

    let next_rule = next.and_then(|n| n.rule.as_ref());
    println!("<details>",);
    println!("<summary>");
    print!(
        "<span class={class:?}>{name}<span/>",
        class = match node.state {
            State::Success => "success",
            State::Failure => "failure",
            State::Unknown => "unknown",
        },
        name = rule.name
    );
    // println!(" at {}:{}", rule.loc.line, rule.loc.column);
    print!(" <code>");

    let before = 20;
    let after = 25;
    print!(
        r#"<em>{}</em>"#,
        &source_file[if rule.loc.pos < before {
            0
        } else {
            rule.loc.pos - before
        }..rule.loc.pos]
    );
    if let Some(next) = next_rule {
        print!(
            r#"<strong>{}</strong>"#,
            &source_file[rule.loc.pos..next.loc.pos]
        );
        print!(
            r#"<span>{}</span>"#,
            &source_file[next.loc.pos..std::cmp::min(next.loc.pos + after, source_file.len())]
        );
    } else {
        print!(
            r#"<span>{}</span>"#,
            &source_file[rule.loc.pos..std::cmp::min(rule.loc.pos + after, source_file.len())]
        );
    }

    println!("</code>");
    println!("</summary>");
    let mut prev_child = None;
    for child in &node.children {
        let child_rule = match child.rule.as_ref() {
            Some(rule) => rule,
            None => continue,
        };
        if child_rule.name == "_" || child_rule.name.ends_with("_guard") {
            continue;
        }

        if let Some(prev) = prev_child {
            visit(prev, Some(child), source_file);
        }
        prev_child = Some(child);
    }
    if let Some(prev) = prev_child {
        visit(prev, next, source_file);
    }

    println!("</details>");
}
