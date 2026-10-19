# ACP node protocol

This document is the wire format for signing and hashing. The terminal client and `acp_rs_node_core` both follow it. There is no protobuf. A field that is added later gets a new version number; old readers ignore an unknown key only after that version says they may.

The `.acpbackup` file is not this format. Its header is format version 1: the bytes `ACPBACKUP`, a length, and one definite CBOR array used as additional data for XChaCha20-Poly1305. That array is not a map, and this document does not change it.

## Canonical CBOR

Encoding follows RFC 8949 section 4.2.1.

- Integers use the shortest form.
- Lengths are definite. Indefinite byte strings, text strings, arrays, and maps are rejected.
- Tags are rejected.
- Floats are rejected.
- The only simple values are `false` (`0xf4`), `true` (`0xf5`), and `null` (`0xf6`).
- Map keys are sorted by their encoded bytes, ascending. A duplicate key is an error.
- A value is one CBOR item. Trailing bytes are an error.

`record_hash` is blake3 of those bytes.

The vectors below are what `acp_rs_node_core::cbor` encodes and accepts. A non-canonical input in the last table is rejected.

### Integers and simple values

| Value | Bytes |
|---|---|
| 0 | `00` |
| 1 | `01` |
| 23 | `17` |
| 24 | `1818` |
| 255 | `18ff` |
| 256 | `190100` |
| -1 | `20` |
| -24 | `37` |
| -25 | `3818` |
| false | `f4` |
| true | `f5` |
| null | `f6` |
| empty byte string | `40` |
| empty text | `60` |
| empty array | `80` |
| empty map | `a0` |
| text `a` | `6161` |
| bytes `01 02` | `420102` |
| array `[1, 2]` | `820102` |

### Maps

Keys sort by encoded bytes, not by their text.

| Value | Bytes |
|---|---|
| `{"b": 1, "a": 2}` | `a2616102616201` |
| `{"aa": 1, "b": 2}` | `a261620262616101` |
| `{10: 1, 2: 3}` | `a202030a01` |

`{"b": 1, "a": 2}` written with `b` first (`a2616201616102`) is rejected. So is a non-shortest integer (`1801` for 1), an indefinite byte string (`5fff`), a tag (`c000`), and a float (`fa00000000`).

### Hash vector

```text
{"n": 24, "type": "vector"}
a2616e1818647479706566766563746f72
blake3 = f1285198df182da88c543a25dbb058a77c5b5887d10e257157141866e9cede8b
```

The key `n` sorts before `type` because `616e` is less than `6474797065`.
