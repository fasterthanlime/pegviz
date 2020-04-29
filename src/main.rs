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
            = "`" name:$(['A'..='Z' | 'a'..='z' | '0'..='9' | '_']*) "` at " line:int() ":" column:int() {
            Rule {
                name: name.into(),
                loc: Location {
                    line,
                    column,
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
    let mut traces: Vec<(Node, String)> = Default::default();
    let mut stack: Vec<Node> = vec![];
    let mut input = String::new();
    let mut trace_number = 1;

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;

        match state {
            ParseState::WaitingForInputStart => {
                if line == "[PEG_INPUT_START]" {
                    println!("=======================================");
                    println!("= pegviz input start");
                    println!("=======================================");
                    state = ParseState::ReadingInput;
                    continue;
                }
            }
            ParseState::ReadingInput => {
                if line == "[PEG_TRACE_START]" {
                    println!("=======================================");
                    println!("= pegviz trace start");
                    println!("=======================================");
                    state = ParseState::ReadingTrace;
                    stack.push(Node {
                        rule: Rule {
                            name: format!("Trace #{}", trace_number),
                            loc: Location { column: 0, line: 0 },
                        },
                        state: State::Success,
                        children: vec![],
                    });
                    trace_number += 1;
                    continue;
                }

                use std::fmt::Write;
                writeln!(&mut input, "{}", line)?;
            }
            ParseState::ReadingTrace => {
                println!("readingtrace, line = {}", line);

                if line == "[PEG_TRACE_STOP]" {
                    println!("=======================================");
                    println!("= pegviz trace stop");
                    println!("=======================================");
                    assert_eq!(stack.len(), 1);
                    let root = stack.pop().unwrap();
                    traces.push((root, input.clone()));
                    input.clear();
                    state = ParseState::WaitingForInputStart;
                    continue;
                }

                let t = match tracer::line(&line) {
                    Ok(t) => t,
                    Err(e) => {
                        println!("=======================================");
                        println!("= pegviz error:\nfor line {}\n{:#?}", line, e);
                        println!("=======================================");
                        return Ok(());
                    }
                };
                println!("read: {:#?}", t);

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

    println!("=======================================");
    println!("= pegviz input stop");
    println!("=======================================");

    if traces.is_empty() {
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
    for trace in &traces {
        let (root, input) = &trace;
        visit(&mut out, root, None, input)?;
    }
    writeln!(
        &mut out,
        r#"
        </body>
    </html>
    "#
    )?;

    println!("=======================================");
    println!("= pegviz generated to {}", args.output.display());
    println!("=======================================");

    Ok(())
}

impl Location {
    fn pos(&self, input: &str) -> usize {
        let mut line = 1;
        let mut column = 1;

        for (i, c) in input.chars().enumerate() {
            if line == self.line && column == self.column {
                return i;
            }

            match c {
                '\n' => {
                    line += 1;
                    column = 0;
                }
                _ => {
                    column += 1;
                }
            }
        }
        0
    }
}

fn visit(
    f: &mut dyn Write,
    node: &Node,
    next: Option<&Node>,
    input: &str,
) -> Result<(), Box<dyn Error>> {
    let rule = &node.rule;

    let next_rule = next.map(|n| &n.rule);
    write!(
        f,
        r#"
    <details>
        <summary>
        <span class="{class}">{name}</span>
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
        &input[if rule.loc.pos(input) < before {
            0
        } else {
            rule.loc.pos(input) - before
        }..rule.loc.pos(input)]
    )?;
    if let Some(next) = next_rule {
        let diff = next.loc.pos(input) - rule.loc.pos(input);
        if diff > 0 {
            write!(
                f,
                r#"<strong>{}</strong>"#,
                &input[rule.loc.pos(input)..next.loc.pos(input)]
            )?;
        }
        write!(
            f,
            r#"<span>{}</span>"#,
            &input[next.loc.pos(input)..std::cmp::min(next.loc.pos(input) + after, input.len())]
        )?;
    } else {
        write!(
            f,
            r#"<span>{}</span>"#,
            &input[rule.loc.pos(input)..std::cmp::min(rule.loc.pos(input) + after, input.len())]
        )?;
    }

    writeln!(f, "</code></summary>")?;
    let mut prev_child = None;
    for child in &node.children {
        // TODO: enable filters?
        // if child.rule.name == "_" || child.rule.name.ends_with("_guard") {
        //     continue;
        // }

        if let Some(prev) = prev_child {
            visit(f, prev, Some(child), input)?;
        }
        prev_child = Some(child);
    }
    if let Some(prev) = prev_child {
        visit(f, prev, next, input)?;
    }
    writeln!(f, "</details>")?;

    Ok(())
}

use ctor::ctor;

#[ctor]
fn install_extensions() {
    color_backtrace::install();
}
