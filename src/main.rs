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
    }

    code {
        color: #fefefe;
        border: 2px solid #777;
        background: #333;
        padding: 2px;
        border-radius: 2px;
        font-family: 'Ubuntu Mono', monospace;
    }

    code strong {
        font-weight: normal;
        color: #777;
    }

    body {
        background: #111;
        color: white;
    }

    details {
        border: 1px solid #333;
        user-select: none;
        cursor: pointer;
        padding-left: 20px;
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
        visit(child, &source_file);
    }
    println!("</body>");
    println!("</html>");

    Ok(())
}

fn visit(node: &Node, source_file: &str) {
    let rule = match node.rule.as_ref() {
        Some(rule) => rule,
        None => return,
    };
    if rule.name == "_" || rule.name.ends_with("_guard") {
        return;
    }

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
    println!(" <code>");
    println!(
        "<strong>{}</strong>",
        &source_file[if rule.loc.pos < 20 {
            0
        } else {
            rule.loc.pos - 20
        }..rule.loc.pos]
    );
    println!(
        "{}",
        &source_file[rule.loc.pos..std::cmp::min(rule.loc.pos + 20, source_file.len())]
    );
    println!("</code>");
    println!("</summary>");
    for child in &node.children {
        visit(child, source_file)
    }
    println!("</details>");
}
