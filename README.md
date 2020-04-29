# pegviz

![](https://img.shields.io/badge/quick%20hack%3F-yes-blue)
![](https://img.shields.io/badge/maintained%3F-not--really-orange)
![](https://img.shields.io/badge/license-mit-blue)

Visualizer for https://crates.io/crates/peg parsers.

## Screenshot

![](https://user-images.githubusercontent.com/7998310/80624360-dccfaf00-8a4b-11ea-967b-5d14607b6592.png)

## Format

`pegviz` expects input in the following format:

```
[PEG_INPUT_START]
typedef int bar;
typedef int bar;

[PEG_TRACE_START]
[PEG_TRACE] Attempting to match rule translation_unit at 1:1 (pos 0)
[PEG_TRACE] Attempting to match rule directive at 1:1 (pos 0)
[PEG_TRACE] Failed to match rule directive at 1:1 (pos 0)
[PEG_TRACE] Attempting to match rule _ at 1:1 (pos 0)
[PEG_TRACE] Matched rule _ at 1:1 (pos 0)
[PEG_TRACE] Attempting to match rule external_declaration at 1:1 (pos 0)
(etc.)
[PEG_TRACE_STOP]
```

The `_START` and `_STOP` marker are pegviz-specific, you'll need to add
them to your program. See the **Integration** section for more information.

Multiple traces may be processed, they'll all show up in the output file.
Output that occurs *between* traces is ignored.

## Compatibility

pegviz has been used with:

  * peg 0.5.7
  * peg 0.6.2

There are no tests. It's quickly thrown together.

## Integration

In your crate, re-export the `trace` feature:

```
# in Cargo.toml

[features]
trace = ["peg/trace"]
```

Then, in your parser, add a `tracing` rule that captures all the input
and outputs the markers `pegviz` is looking for:

```rust
peg::parser! { pub grammar example() for str {

rule traced<T>(e: rule<T>) -> T =
    &(input:$([_]*) {
        #[cfg(feature = "trace")]
        println!("[PEG_INPUT_START]\n{}\n[PEG_TRACE_START]", input);
    })
    e:e()? {?
        #[cfg(feature = "trace")]
        println!("[PEG_TRACE_STOP]");
        e.ok_or("")
    }

pub rule toplevel() -> Toplevel = traced(<toplevel0()>)

}}
```

The above is the recommended way *if you're maintaining the grammar* and want
to be able to turn on pegviz support anytime.

If you're debugging someone else's parser, you may want to print the start/stop
markers and the source yourself, around the parser invocation, like so:

```rust
    let source = std::fs::read_to_string(&source).unwrap();
    println!("[PEG_INPUT_START]\n{}\n[PEG_TRACE_START]", source);
    let res = lang_c::driver::parse_preprocessed(&config, source);
    println!("[PEG_TRACE_STOP]");
```

Make sure you've installed `pegviz` into your `$PATH`:

```shell
cd pegviz/
cargo install --force --path .
```

> While installing it, you may notice `pegviz` depends on `peg`.
> That's right! It's using a PEG to analyze PEG traces.

Then, simply run your program with the `trace` Cargo feature enabled, and
pipe its standard output to `pegviz`.

```shell
cd example/
cargo run --features trace | pegviz --output ./pegviz.html
```

Note that the `--output` argument is mandatory.

The last step is to open the resulting HTML file in a browser and click around!

## License

pegviz is released under the MIT License. See the LICENSE file for details.
