use argh::FromArgs;
use std::{
    error::Error,
    fs::File,
    io::{BufRead, Write},
    path::PathBuf,
};

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
    rule: Rule,
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

#[derive(FromArgs)]
/// Creates an HTML visualization for a trace generated from https://crates.io/crates/peg
struct Args {
    #[argh(option, short = 'o')]
    /// output path, "./trace.html" for example
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Args = argh::from_env();

    enum ParseState {
        WaitingForInputStart,
        ReadingInput,
        ReadingTrace,
    }
    let mut state = ParseState::WaitingForInputStart;
    let mut stack: Vec<Node> = vec![Node {
        rule: Rule {
            name: "Root".into(),
            loc: Location {
                column: 0,
                line: 0,
                pos: 0,
            },
        },
        state: State::Success,
        children: vec![],
    }];
    let mut input = String::new();

    let stdin = std::io::stdin();
    'each_line: for line in stdin.lock().lines() {
        let line = line?;

        match state {
            ParseState::WaitingForInputStart => {
                if line == "[PEG_INPUT_START]" {
                    state = ParseState::ReadingInput;
                }
            }
            ParseState::ReadingInput => {
                if line == "[PEG_TRACE_START]" {
                    state = ParseState::ReadingTrace;
                } else {
                    use std::fmt::Write;
                    writeln!(&mut input, "{}", line)?;
                }
            }
            ParseState::ReadingTrace => {
                let t = match tracer::line(&line) {
                    Ok(t) => t,
                    Err(_) => break 'each_line,
                };

                match t {
                    Line::Attempt(rule) => {
                        let node = Node {
                            rule,
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
        }
    }

    if input.is_empty() {
        println!("pegviz: empty code, exiting");
        return Ok(());
    }

    let root = stack.pop().unwrap();
    if root.children.is_empty() {
        println!("pegviz: no trace, exiting");
        return Ok(());
    }

    let mut out = File::create(&args.output)?;

    writeln!(
        &mut out,
        r#"
    <html>
        <head>
            <style>{style}</style>
        </head>
        <body>
    "#,
        style = include_str!("style.css")
    )?;
    for child in &root.children {
        visit(&mut out, child, None, &input)?;
    }
    writeln!(
        &mut out,
        r#"
        </body>
    </html>
    "#
    )?;

    Ok(())
}

fn visit(
    f: &mut dyn Write,
    node: &Node,
    next: Option<&Node>,
    code: &str,
) -> Result<(), Box<dyn Error>> {
    let rule = &node.rule;

    let next_rule = next.map(|n| &n.rule);
    write!(
        f,
        r#"
    <details>
        <summary>
        <span class="{class:?}">{name}</span>
        <code>"#,
        class = match node.state {
            State::Success => "success",
            State::Failure => "failure",
            State::Unknown => "unknown",
        },
        name = rule.name
    )?;

    let before = 20;
    let after = 25;
    write!(
        f,
        r#"<em>{}</em>"#,
        &code[if rule.loc.pos < before {
            0
        } else {
            rule.loc.pos - before
        }..rule.loc.pos]
    )?;
    if let Some(next) = next_rule {
        write!(
            f,
            r#"<strong>{}</strong>"#,
            &code[rule.loc.pos..next.loc.pos]
        )?;
        write!(
            f,
            r#"<span>{}</span>"#,
            &code[next.loc.pos..std::cmp::min(next.loc.pos + after, code.len())]
        )?;
    } else {
        write!(
            f,
            r#"<span>{}</span>"#,
            &code[rule.loc.pos..std::cmp::min(rule.loc.pos + after, code.len())]
        )?;
    }

    writeln!(f, "</code></summary>")?;
    let mut prev_child = None;
    for child in &node.children {
        if child.rule.name == "_" || child.rule.name.ends_with("_guard") {
            continue;
        }

        if let Some(prev) = prev_child {
            visit(f, prev, Some(child), code)?;
        }
        prev_child = Some(child);
    }
    if let Some(prev) = prev_child {
        visit(f, prev, next, code)?;
    }
    writeln!(f, "</details>")?;

    Ok(())
}
