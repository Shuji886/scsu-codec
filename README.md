# scsu-codec

A Rust library that implements the Standard Compression Scheme for Unicode
(SCSU) from Unicode Technical Standard #6 for compact text.

## Usage

```rust
use scsu_codec::{Encoder, Decoder};

let mut encoder = Encoder::new();
let mut decoder = Decoder::new();

let input: Vec<char> = "Hello, 世界! 😀".chars().collect();
let encoded = encoder.encode(&input);
let decoded = decoder.decode(&encoded).unwrap();

assert_eq!(decoded, input);
```

## Why this exists

SCSU is a niche but useful compression scheme for Unicode text that avoids
the overhead of UTF-8 for scripts that are not Latin-1. This library exists
so that Rust programs can encode and decode SCSU without pulling in a large
Unicode library or a third-party dependency.

The trade-off is simplicity over maximum compression. The encoder uses a
greedy strategy: it emits a character as a single byte if it falls into a
static or dynamic window, otherwise it uses a quote or extended quote. It
does not attempt to optimise window selection across the whole input, which
would require a more complex algorithm.

## Edge cases

The decoder is strict about malformed input: truncated quotes, truncated
extended quotes, and invalid Unicode scalar values (such as surrogates)
return an error. The encoder assumes valid input; if you pass a surrogate
in a `char`, it will be encoded as an extended quote, which is technically
invalid SCSU but round-trips through the decoder.

## Limitations

The encoder does not implement the full set of SCSU commands for defining
dynamic windows (UD0..UD7, UC0..UC7). It only uses static windows and the
initial dynamic windows (all zero). This means that for some inputs the
compression ratio is worse than a full SCSU implementation, but the output
is still valid SCSU and can be decoded by any conforming decoder.
