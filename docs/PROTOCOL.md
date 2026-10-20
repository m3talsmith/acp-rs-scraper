# ACP node protocol

This document is the wire format for signing and hashing. The terminal client (`acp-red-star`, repository root) and `acp_red_star_node_core` (`crates/acp_red_star_node_core`) both follow it. There is no protobuf. A field that is added later gets a new version number; old readers ignore an unknown key only after that version says they may.

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

The vectors below are what `acp_red_star_node_core::cbor` encodes and accepts. A non-canonical input in the last table is rejected.

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

## Peer id

A node's libp2p identity is the ed25519 signing public key. There is no protobuf dependency. The bytes that libp2p hashes are the protobuf encoding of that key, written by hand:

```text
08 01          field 1, varint, key type Ed25519 = 1
12 20          field 2, length-delimited, 32 bytes
<32-byte public key>
```

The peer id is base58btc of the sha2-256 multihash of those bytes: `12 20` followed by the 32-byte hash. A 34-byte multihash is short enough that the peer id is the multihash itself.

Vector. Public key `11` repeated 32 times:

```text
fingerprint = 4a78877c4a0926fc
peer id     = QmXS53cQyDRT7RaXiKYLjfkX8xSc9pBDPohDh1F3HxzjAz
```

The fingerprint is the first 8 bytes of blake3(public key), hex encoded.

## Certificates

Certificates are canonical CBOR maps. ed25519 signs the canonical bytes of the map. The signature is not a field of the map. Version is `v = 1`.

An authorizer certificate has `type = "authorizer"`, `account` (master signing public key), `subject` (the authorizer's node signing public key), `serial`, `issued_at`, `expires_at`, and `limits`. The limits map has `max_nodes`, `max_node_ttl`, `history_grant` (`allowed` or `forbidden`), and `may_revoke` (`own`).

A node certificate has `type = "node"`, `account`, `node_sign_pub`, `node_enc_pub`, `peer_id`, `label`, `serial`, `issuer`, `issued_at`, `expires_at`, and `caps`. The first machine's capabilities are `intent-sender`, `publish`, and `sync`, sorted as text.

The first machine uses serial 1 for the authorizer and serial 2 for the node. The master signs both. The node certificate's issuer is the master public key, so the chain is node → master. The authorizer certificate is what lets this machine certify a later node.

Decided in the plan (2026-10-19) and compiled in 3.3.2: an authorizer certificate lasts 30 days, a node certificate lasts 365 days, and a verifier allows 5 minutes of clock skew. Renewal, the 7-day grace period, and revocation are not built yet. 3.3.0 and 3.3.1 had the two lifetimes reversed. A node from those versions keeps that certificate until `~/.acp/node` is removed and `--node-init` is run again, which creates a new node key. On the first machine `history_grant` is `allowed` and `max_nodes` is 16. Those two limits are accepted.

A verifier checks the signature, that the chain ends at the pinned master, that `now` is inside the validity window plus the skew, and, when a capability is required, that it is in `caps`. A node certificate that names a different account is rejected.

## Verification words

An enrollment request is a canonical map: `type = "enroll-request"`, `v = 1`, `sign`, `enc`, `label`, `nonce` (16 bytes). The six words are BIP-39 English words from the first 66 bits of blake3 of that encoding. Word 0 is the top 11 bits.

Vector. Signing key seed `11` repeated, wrapping seed `22` repeated, label `home`, nonce `03` repeated 16 times:

```text
zebra cloud van any butter science
```

## Tombstones and restore

A tombstone does not delete the post. The node keeps the body. Never let data go to waste. The UI shows the post as deleted with a tombstone icon (the glyph 🪦, or `[RIP]` when the terminal cannot render it). The content stays viewable.

A restore record is a canonical CBOR map signed by the author's key. The signature is over the canonical bytes of the map and is not a field of the map. Version is `v = 1`.

Fields: `type = "restore"`, `v`, `post_id`, `tombstone` (the `record_hash` of the tombstone this retracts), `issued_at` (unix seconds).

A newer timestamp wins. If `issued_at` is later than the tombstone it names, the tombstone is cleared and the post displays normally again. The retained body is what comes back. A post record for the same `post_id` signed by the author with a newer timestamp does the same thing: the tombstone is cleared and the post id is restored. An older restore record does not clear a newer tombstone.

## Node keystore

`~/.acp/node/keystore` is not a certificate and it is not inside a `.acpbackup`. The file is `ACPKEY01`, then a 24-byte nonce, then XChaCha20-Poly1305 ciphertext. The additional data is `acp-node-keystore-v1`. The key is HKDF-SHA256 of the unlocked database key with info `acp-node-keystore-v1`. The plaintext is canonical CBOR with `v`, `sign` (32-byte seed), and `enc` (32-byte seed). A wrong key fails the authenticator. The file mode is `0600`.
