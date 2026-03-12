# RNES

This is really just a way for me to try and develop my Rust skills with a
fun project. Inspired by Matt Godbolt's [specbolt](https://github.com/mattgodbolt/specbolt),
I also wanted to make good use of various Rust features such as async (an
atypical use implementing emulated system cycles).

This project is still in its infancy so most ROMs will fail. Currently mapper 0
will just barely work but not reliably.

## Prerequisites

- Requires Rust 1.85.1

## Running

`cargo run --release -- <your favourite ROM here>`