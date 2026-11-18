#!/usr/bin/env python3
"""Pack data/font_metrics_data.json into src/generated/font_metrics_packed.bin (used with the `packed-metrics` feature).

    python3 -I crates/katex/tools/gen_packed_metrics.py

Layout: the fonts in the order of `FONT_METRICS_INDEX` (sorted by name), for each its entry count as a u16; then for every font in turn, as
columns of LEB128 numbers: the key deltas (the first from 0), then depth, height, italic, skew, width of every entry, each as a zig-zag
number in units of 1/100000 (every metric of KaTeX's tables is a multiple of that: the script checks it, and that n / 1e5 is the same f64 as
the decimal in the source). One column per field and font compresses far better than the array of structs of `generated/font_metrics_data.rs`.
Uses only the Python standard library.
"""
import json
import os
import struct

CRATE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
data = json.load(open(os.path.join(CRATE, "data", "font_metrics_data.json")))
names = sorted(data)


def varint(n):
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def zigzag(n):
    return (n << 1) if n >= 0 else ((-n << 1) - 1)


head = b""
body = b""
for name in names:
    entries = sorted((int(k), v) for k, v in data[name].items())
    head += struct.pack("<H", len(entries))
    cols = [bytearray() for _ in range(6)]
    prev = 0
    for key, vals in entries:
        cols[0] += varint(key - prev)
        prev = key
        for j, x in enumerate(vals):
            n = round(x * 100000)
            assert n / 100000.0 == float(x) and repr(n / 100000.0) == repr(float(x)), (name, key, x)
            cols[j + 1] += varint(zigzag(n))
    body += b"".join(bytes(c) for c in cols)
open(os.path.join(CRATE, "src", "generated", "font_metrics_packed.bin"), "wb").write(head + body)
print(len(names), "fonts,", sum(len(v) for v in data.values()), "entries,", len(head + body), "bytes")
print("names:", names)
