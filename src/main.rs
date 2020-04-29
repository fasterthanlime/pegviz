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
    Cache,
    EnterLevel,
    LeaveLevel,
}

peg::parser! {
    grammar tracer() for str {
        pub(crate) rule line() -> Line
            = "[PEG_TRACE] " l:line0() { l }

        rule line0() -> Line
            = a:attempt() { Line::Attempt(a) }
            / cach() { Line::Cache }
            / enter() { Line::EnterLevel }
            / leave() { Line::LeaveLevel }
            / fail() { Line::Failure }
            / succ() { Line::Success }

        rule fail()
            = "Failed to match rule " [_]*

        rule cach()
            = "Cached match of rule " [_]*

        rule succ()
            = "Matched rule " [_]*

        rule enter()
            = "Entering level " [_]*

        rule leave()
            = "Leaving level " [_]*

        rule attempt() -> Rule
            = "Attempting to match rule " name:name() " at " loc:location() (" (pos " int() ")")? {
                Rule {
                    name: name.into(),
                    loc,
                }
            }

        rule backquoted<T>(e: rule<T>) -> T
            = "`" e:e() "`" { e }

        rule name() -> &'input str
            = n:backquoted(<identifier()>) { n }
            / n:identifier() { n }

        rule identifier() -> &'input str
            = $(['A'..='Z' | 'a'..='z' | '0'..='9' | '_']*)

        rule location() -> Location
            = line:int() ":" column:int() { Location { line, column } }

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

    #[argh(option)]
    /// name of rules to flatten - if they have only a single child,
    /// then only the child will appear in the tree
    flatten: Vec<String>,

    #[argh(option)]
    /// name of rules to hide altogether
    hide: Vec<String>,
}

impl Args {
    fn should_flatten(&self, node: &Node) -> bool {
        self.flatten
            .iter()
            .find(|&x| x == &node.rule.name)
            .is_some()
            && node.children.len() == 1
    }

    fn should_hide(&self, node: &Node) -> bool {
        self.hide.iter().find(|&x| x == &node.rule.name).is_some()
    }
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
                    println!("= pegviz input start");
                    state = ParseState::ReadingInput;
                    continue;
                }
            }
            ParseState::ReadingInput => {
                if line == "[PEG_TRACE_START]" {
                    println!("= pegviz trace start");
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
                if line == "[PEG_TRACE_STOP]" {
                    println!("= pegviz trace stop");
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
                        println!("= pegviz error:\nfor line\n|  {}\n{:#?}", line, e);
                        return Ok(());
                    }
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
                    Line::Cache => {}
                    Line::EnterLevel => {}
                    Line::LeaveLevel => {}
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
        visit(&mut out, &args, root, None, input)?;
    }
    writeln!(
        &mut out,
        r#"
        </body>
    </html>
    "#
    )?;

    println!("= pegviz generated to {}", args.output.display());

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
                    column = 1;
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
    args: &Args,
    node: &Node,
    next: Option<&Node>,
    input: &str,
) -> Result<(), Box<dyn Error>> {
    if args.should_flatten(node) {
        return visit(f, args, &node.children[0], next, input);
    }

    let rule = &node.rule;

    let next_rule = next.map(|n| &n.rule);
    write!(
        f,
        r#"
    <details>
        <summary>
        <span class="rule {class}">{name}</span>
        <code>"#,
        class = match node.state {
            State::Success => "success",
            State::Failure => "failure",
            State::Unknown => "unknown",
        },
        name = rule.name
    )?;

    let before = 10;
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
    let rulepos = rule.loc.pos(input);
    if let Some(next) = next_rule {
        let nextpos = next.loc.pos(input);
        if nextpos > rulepos {
            write!(f, r#"<strong>{}</strong>"#, &input[rulepos..nextpos])?;
        } else if nextpos < rulepos {
            write!(f, r#"↩"#)?;
        }
        write!(
            f,
            r#"<span>{}{}</span>"#,
            &input[nextpos..std::cmp::min(nextpos + after, input.len())],
            if input.len() > nextpos + after {
                "…"
            } else {
                ""
            }
        )?;
    } else {
        write!(
            f,
            r#"<span>{}{}</span>"#,
            &input[rulepos..std::cmp::min(rulepos + after, input.len())],
            if input.len() > rulepos + after {
                "…"
            } else {
                ""
            }
        )?;
    }

    writeln!(f, "</code></summary>")?;
    let mut prev_child = None;
    for child in &node.children {
        if args.should_hide(child) {
            continue;
        }

        if let Some(prev) = prev_child {
            visit(f, args, prev, Some(child), input)?;
        }
        prev_child = Some(child);
    }
    if let Some(prev) = prev_child {
        visit(f, args, prev, next, input)?;
    }
    writeln!(f, "</details>")?;

    Ok(())
}

use ctor::ctor;

#[ctor]
fn install_extensions() {
    color_backtrace::install();
}
