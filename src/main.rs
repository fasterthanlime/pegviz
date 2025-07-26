use argh::FromArgs;
use std::{
    cmp::Ordering,
    error::Error,
    fmt,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};

#[derive(Debug, Clone)]
enum State {
    Success,
    Failure,
    Unknown,
}

// Location is the position of a statement in the source
// text or slice.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum Location {
    CharLocation(CharLocation),
    TokenIndex(TokenIndex),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharLocation {
    line: usize,
    column: usize,
}

impl fmt::Display for CharLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

impl Ord for CharLocation {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.line.cmp(&other.line) {
            Ordering::Equal => self.column.cmp(&other.column),
            x => x,
        }
    }
}

impl PartialOrd for CharLocation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TokenIndex {
    index: usize,
}

impl fmt::Display for TokenIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.index)
    }
}

#[derive(Debug)]
struct Node {
    rule: Rule,
    partial_match: bool,
    state: State,
    children: Vec<Node>,
}

#[derive(Debug)]
struct Rule {
    name: String,
    loc: Location,
    next_loc: Option<Location>,
}

impl Rule {
    #[allow(dead_code)]
    fn is_zero_len(&self) -> bool {
        if let Some(next_loc) = self.next_loc {
            if next_loc > self.loc {
                return false;
            }
        }
        true
    }
}

#[derive(Debug)]
enum Line {
    Attempt(Rule),
    Failure(Rule),
    Success(Rule),
    Cache,
    EnterLevel,
    LeaveLevel,
}

peg::parser! {
    grammar tracer() for str {
        pub(crate) rule line() -> Line
            = "[PEG_TRACE] " l:line0() { l }

        rule line0() -> Line
            = r:attempt() { Line::Attempt(r) }
            / r:fail() { Line::Failure(r) }
            / r:succ() { Line::Success(r) }
            / cach() { Line::Cache }
            / enter() { Line::EnterLevel }
            / leave() { Line::LeaveLevel }

        rule attempt() -> Rule
            = "Attempting to match rule " r:rule0() { r }

        rule fail() -> Rule
            = "Failed to match rule " r:rule0() { r }

        rule succ() -> Rule
            = "Matched rule " r:rule0() { r }

        rule cach()
            = "Cached " ("match" / "fail") " of rule " [_]*

        rule enter()
            = "Entering level " [_]*

        rule leave()
            = "Leaving level " [_]*

        rule rule0() -> Rule
            = rule1(<identifier()>, <at5()>)
            / rule1(<backquoted(<identifier()>)>, <at6()>)

        rule rule1(name: rule<&'input str>, at: rule<(Location, Option<Location>)>) -> Rule
            = name:name() at:at() {
                Rule {
                    name: name.into(),
                    loc: at.0,
                    next_loc: at.1,
                }
            }

        rule at5() -> (Location, Option<Location>)
            = " at " at:location() " (pos " int() ")" { (at, None) }

        rule at6() -> (Location, Option<Location>)
            = " at " at:location() to:(" to " to:location() { to })? { (at, to) }

        rule backquoted<T>(e: rule<T>) -> T
            = "`" e:e() "`" { e }

        rule identifier() -> &'input str
            = $(['A'..='Z' | 'a'..='z' | '0'..='9' | '_']*)

        rule location() -> Location
            = range_location() / index_location()

        rule range_location() -> Location
            = line:int() ":" column:int() { Location::CharLocation(CharLocation { line, column } ) }

        rule index_location() -> Location
            = index:int() { Location::TokenIndex(TokenIndex { index } ) }

        rule int() -> usize
            = digits:$(['0'..='9']+) { digits.parse().unwrap() }
    }
}

#[derive(FromArgs)]
/// Creates an HTML visualization for a trace generated from https://crates.io/crates/peg
struct Args {
    #[argh(positional)]
    input: Option<PathBuf>,

    #[argh(option, short = 'o')]
    /// output path, "./trace.html" for example
    output: PathBuf,

    #[argh(option, short = 'f')]
    /// name of rules to flatten - if they have only a single child,
    /// then only the child will appear in the tree
    flatten: Vec<String>,

    #[argh(option, short = 'h')]
    /// name of rules to hide altogether
    hide: Vec<String>,
}

impl Args {
    fn should_flatten(&self, node: &Node) -> bool {
        self.flatten.iter().any(|x| x == &node.rule.name) && node.children.len() == 1
    }

    fn should_hide(&self, node: &Node) -> bool {
        self.hide.iter().any(|x| x == &node.rule.name)
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
    let stream = match &args.input {
        Some(input) => Box::new(BufReader::new(File::open(input)?)) as Box<dyn BufRead>,
        None => Box::new(stdin.lock()) as Box<dyn BufRead>,
    };

    for line in stream.lines() {
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
                            loc: Location::CharLocation(CharLocation { column: 0, line: 0 }),
                            next_loc: None,
                        },
                        partial_match: false,
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
                    let mut root = stack.pop().unwrap();
                    let child = &root.children[0];
                    root.state = child.state.clone();
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
                            partial_match: false,
                        };
                        stack.push(node);
                    }
                    Line::Success(rule) => {
                        let mut node = stack.pop().unwrap();
                        if rule.name != node.rule.name {
                            panic!(
                                "pegviz: expected rule {:?} to finish, but got {:?}",
                                rule.name, node.rule.name
                            );
                        }
                        node.state = State::Success;
                        node.rule.next_loc = rule.next_loc;
                        stack.last_mut().unwrap().children.push(node);
                    }
                    Line::Failure(rule) => {
                        let mut node = stack.pop().unwrap();
                        if rule.name != node.rule.name {
                            panic!(
                                "pegviz: expected rule {:?} to finish, but got {:?}",
                                rule.name, node.rule.name
                            );
                        }
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
    <!DOCTYPE html>
    <html lang="en">
        <head>
        <meta charset="utf-8"/>
            <style>{style}</style>
            <script>{script}</script>
        </head>
        <body>
        <div id="notifications"></div>
    "#,
        style = include_str!("style.css"),
        script = include_str!("index.js")
    )?;

    for trace in &mut traces {
        backfill_next_loc(&mut trace.0, None);
        mark_partial_matches(&mut trace.0);
    }

    for trace in &traces {
        let (root, input) = &trace;
        visit(&mut out, &args, root, input)?;
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

#[allow(unused)]
fn print_backfilled(node: &Node, state: &str) {
    #[cfg(feature = "debug-backfill")]
    {
        if node.rule.is_zero_len() {
            return;
        }

        if let Some(next_loc) = node.rule.next_loc {
            println!(
                "{name:?} {state}: {from}-{to}",
                name = node.rule.name,
                state = state,
                from = node.rule.loc,
                to = next_loc,
            );
        }
    }
}

fn mark_partial_matches(node: &mut Node) {
    for c in &mut node.children {
        mark_partial_matches(c);
    }

    let ret = (matches!(node.state, State::Success) && !node.rule.is_zero_len())
        || node.children.iter().any(|c| c.partial_match);
    node.partial_match = ret;
}

fn backfill_next_loc(node: &mut Node, next: Option<&Node>) {
    for i in 1..node.children.len() {
        if let ([prev], [next]) = &mut node.children[i - 1..i + 1].split_at_mut(1) {
            if prev.rule.next_loc.is_none() {
                prev.rule.next_loc = Some(next.rule.loc);
                print_backfilled(prev, "backfilled");
            } else {
                print_backfilled(prev, "parsed");
            }
            backfill_next_loc(prev, Some(next));
        }
    }

    if let Some(last) = node.children.last_mut() {
        if let Some(next) = next {
            if last.rule.next_loc.is_none() {
                last.rule.next_loc = Some(next.rule.loc);
                print_backfilled(last, "backfilled");
            } else {
                print_backfilled(last, "parsed");
            }
        }
        backfill_next_loc(last, next)
    }
}

impl Location {
    fn pos(&self, input: &str) -> usize {
        match self {
            Location::CharLocation(char_loc) => char_loc.pos(input),
            Location::TokenIndex(tok_idx) => tok_idx.pos(input),
        }
    }
}

impl CharLocation {
    // returns the index of the character at the specified line and column
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

impl TokenIndex {
    // returns the index of the character at the specified line and column
    fn pos(&self, input: &str) -> usize {
        let mut line = 0;

        for (i, c) in input.chars().enumerate() {
            if line == self.index {
                return i;
            }
            if c == '\n' {
                line += 1;
            }
        }
        0
    }
}

fn visit(f: &mut dyn Write, args: &Args, node: &Node, input: &str) -> Result<(), Box<dyn Error>> {
    if args.should_flatten(node) {
        return visit(f, args, &node.children[0], input);
    }

    let rule = &node.rule;

    write!(
        f,
        r#"
    <details>
        <summary>
        <span class="rule {class} {class2}">{name}</span>
        <code>"#,
        class = match node.state {
            State::Success => "success",
            State::Failure => "failure",
            State::Unknown => "unknown",
        },
        class2 = if node.partial_match {
            "partial-match"
        } else {
            ""
        },
        name = rule.name
    )?;

    // Print up to 10 characters before and 25 characters after
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
    if let Some(next_loc) = rule.next_loc.as_ref() {
        let nextpos = next_loc.pos(input);
        match nextpos.cmp(&rulepos) {
            Ordering::Greater => {
                write!(f, r#"<strong>{}</strong>"#, &input[rulepos..nextpos])?;
            }
            Ordering::Less => {
                write!(f, r#"↩"#)?;
            }
            Ordering::Equal => {}
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
    for child in &node.children {
        if args.should_hide(child) {
            continue;
        }
        visit(f, args, child, input)?;
    }
    writeln!(f, "</details>")?;

    Ok(())
}
