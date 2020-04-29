# pegviz

Visualizer for https://crates.io/crates/peg 0.6.x parsers.

## Usage

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

After installing pegviz to your path:

```shell
cd pegviz/
cargo install --force --path .
```

...simply run your program with the `trace` Cargo feature enabled, and pipe
its standard output to `pegviz`, and specifying an output file:

```shell
cd example/
cargo run --features trace | pegviz --output ./pegviz.html
```

Then open `pegviz.html` in a browser.

## Screenshot

![](https://user-images.githubusercontent.com/7998310/80550548-b6b0fd00-89c0-11ea-8c47-ee1cee972aeb.png)
