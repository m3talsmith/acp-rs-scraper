//! SLIP-39 (Shamir) shares, one group, 3 of 5, with an empty share passphrase.
//!
//! The account password is not an input. Three shares recover the 256-bit seed
//! and nothing more; the master secret still needs the password. The published
//! `slip39` crate does not build (it depends on a yanked `bitvec`), so this is
//! the SLIP-39 construction itself, checked against the official test vectors.
//!
//! New shares set the extendable flag (`ext = 1`) as the spec requires. Reading
//! still accepts `ext = 0`.

use anyhow::{Result, bail};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

const WORDLIST: &str = include_str!("slip39_wordlist.txt");
const RADIX: usize = 1024;
const SECRET_INDEX: u8 = 255;
const DIGEST_INDEX: u8 = 254;

type HmacSha256 = Hmac<Sha256>;

fn words() -> Vec<&'static str> {
    WORDLIST
        .lines()
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect()
}

fn word_index(word: &str) -> Result<u16> {
    let list = words();
    list.binary_search(&word)
        .map(|i| i as u16)
        .map_err(|_| anyhow::anyhow!("unknown share word {word}"))
}

fn customization(ext: u8) -> &'static str {
    if ext == 0 {
        "shamir"
    } else {
        "shamir_extendable"
    }
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

fn gf_inv(a: u8) -> u8 {
    // a^(254) in GF(256).
    let mut r = 1u8;
    let mut base = a;
    let mut exp = 254u32;
    while exp > 0 {
        if exp & 1 == 1 {
            r = gf_mul(r, base);
        }
        base = gf_mul(base, base);
        exp >>= 1;
    }
    r
}

fn interpolate(x: u8, points: &[(u8, &[u8])]) -> Vec<u8> {
    let n = points[0].1.len();
    let mut out = vec![0u8; n];
    for (i, (xi, yi)) in points.iter().enumerate() {
        let mut num = 1u8;
        let mut den = 1u8;
        for (j, (xj, _)) in points.iter().enumerate() {
            if i == j {
                continue;
            }
            num = gf_mul(num, x ^ xj);
            den = gf_mul(den, xi ^ xj);
        }
        let coeff = gf_mul(num, gf_inv(den));
        for (k, byte) in yi.iter().enumerate() {
            out[k] ^= gf_mul(*byte, coeff);
        }
    }
    out
}

fn hmac4(key: &[u8], msg: &[u8]) -> [u8; 4] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(msg);
    let full = mac.finalize().into_bytes();
    let mut out = [0u8; 4];
    out.copy_from_slice(&full[..4]);
    out
}

fn split_secret(threshold: usize, count: usize, secret: &[u8]) -> Result<Vec<Vec<u8>>> {
    if threshold == 0 || threshold > count || count > 16 {
        bail!("share threshold must be between 1 and the share count, at most 16");
    }
    if secret.len() < 16 || secret.len() % 2 != 0 {
        bail!("a shared secret must be at least 128 bits and an even number of bytes");
    }
    if threshold == 1 {
        return Ok(vec![secret.to_vec(); count]);
    }
    let n = secret.len();
    let mut r = vec![0u8; n - 4];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut r);
    let digest = hmac4(&r, secret);
    let mut d = Vec::with_capacity(n);
    d.extend_from_slice(&digest);
    d.extend_from_slice(&r);
    let mut randoms: Vec<Vec<u8>> = Vec::new();
    for _ in 0..threshold - 2 {
        let mut y = vec![0u8; n];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut y);
        randoms.push(y);
    }
    let mut base: Vec<(u8, Vec<u8>)> = Vec::new();
    for (i, y) in randoms.iter().enumerate() {
        base.push((i as u8, y.clone()));
    }
    base.push((DIGEST_INDEX, d));
    base.push((SECRET_INDEX, secret.to_vec()));
    let view: Vec<(u8, &[u8])> = base.iter().map(|(x, y)| (*x, y.as_slice())).collect();
    let mut shares = randoms.clone();
    for x in (threshold - 2)..count {
        shares.push(interpolate(x as u8, &view));
    }
    Ok(shares)
}

fn recover_secret(threshold: usize, parts: &[(u8, &[u8])]) -> Result<Vec<u8>> {
    if parts.is_empty() {
        bail!("no shares");
    }
    if threshold == 1 {
        return Ok(parts[0].1.to_vec());
    }
    if parts.len() < threshold {
        bail!("need {threshold} shares, got {}", parts.len());
    }
    let secret = interpolate(SECRET_INDEX, parts);
    let digest_share = interpolate(DIGEST_INDEX, parts);
    if digest_share.len() < 4 {
        bail!("share digest is short");
    }
    let r = &digest_share[4..];
    let expect = hmac4(r, &secret);
    if expect.as_slice() != &digest_share[..4] {
        bail!("these shares do not reconstruct a consistent secret");
    }
    Ok(secret)
}

fn pbkdf(password: &[u8], salt: &[u8], rounds: u32, out: &mut [u8]) {
    pbkdf2::pbkdf2_hmac::<Sha256>(password, salt, rounds, out);
}

fn feistel(block: &[u8], passphrase: &str, e: u8, id: u16, ext: u8, decrypt: bool) -> Vec<u8> {
    let n = block.len();
    let half = n / 2;
    let mut l = block[..half].to_vec();
    let mut r = block[half..].to_vec();
    let rounds: [u8; 4] = if decrypt { [3, 2, 1, 0] } else { [0, 1, 2, 3] };
    let iters = 2500u32 << e;
    for i in rounds {
        let mut password = Vec::with_capacity(1 + passphrase.len());
        password.push(i);
        password.extend(passphrase.as_bytes());
        let mut salt = Vec::new();
        if ext == 0 {
            salt.extend_from_slice(b"shamir");
            salt.extend_from_slice(&id.to_be_bytes());
        }
        salt.extend_from_slice(&r);
        let mut f = vec![0u8; half];
        pbkdf(&password, &salt, iters, &mut f);
        let mut new_r = l.clone();
        for (b, k) in new_r.iter_mut().zip(f.iter()) {
            *b ^= k;
        }
        l = r;
        r = new_r;
    }
    let mut out = Vec::with_capacity(n);
    if decrypt {
        out.extend_from_slice(&r);
        out.extend_from_slice(&l);
    } else {
        // EMS = R || L after the forward rounds.
        out.extend_from_slice(&r);
        out.extend_from_slice(&l);
    }
    out
}

fn encrypt_master(ms: &[u8], passphrase: &str, e: u8, id: u16, ext: u8) -> Vec<u8> {
    feistel(ms, passphrase, e, id, ext, false)
}

fn decrypt_master(ems: &[u8], passphrase: &str, e: u8, id: u16, ext: u8) -> Vec<u8> {
    feistel(ems, passphrase, e, id, ext, true)
}

struct Bits(Vec<u8>);

impl Bits {
    fn push(&mut self, value: u32, n: u32) {
        for i in (0..n).rev() {
            self.0.push(((value >> i) & 1) as u8);
        }
    }
    fn push_share_value(&mut self, value: &[u8]) {
        let nbits = value.len() * 8;
        let pad = (10 - (nbits % 10)) % 10;
        self.push(0, pad as u32);
        for b in value {
            self.push(*b as u32, 8);
        }
    }
    fn words(&self) -> Vec<u16> {
        self.0
            .chunks(10)
            .map(|c| c.iter().fold(0u16, |a, b| (a << 1) | u16::from(*b)))
            .collect()
    }
}

fn rs1024_polymod(values: &[u32]) -> u32 {
    const GEN: [u32; 10] = [
        0x00e0_e040,
        0x01c1_c080,
        0x0383_8100,
        0x0707_0200,
        0x0e0e_0009,
        0x1c0c_2412,
        0x3808_6c24,
        0x3090_fc48,
        0x21b1_f890,
        0x03f3_f120,
    ];
    let mut chk = 1u32;
    for &v in values {
        let b = chk >> 20;
        chk = ((chk & 0x000f_ffff) << 10) ^ v;
        for (i, coeff) in GEN.iter().enumerate() {
            if (b >> i) & 1 == 1 {
                chk ^= coeff;
            }
        }
    }
    chk
}

fn checksum_words(cs: &str, data: &[u16]) -> [u16; 3] {
    let mut values: Vec<u32> = cs.bytes().map(u32::from).collect();
    values.extend(data.iter().copied().map(u32::from));
    values.extend_from_slice(&[0, 0, 0]);
    let polymod = rs1024_polymod(&values) ^ 1;
    [
        ((polymod >> 20) & 1023) as u16,
        ((polymod >> 10) & 1023) as u16,
        (polymod & 1023) as u16,
    ]
}

fn checksum_ok(cs: &str, data: &[u16]) -> bool {
    let mut values: Vec<u32> = cs.bytes().map(u32::from).collect();
    values.extend(data.iter().copied().map(u32::from));
    rs1024_polymod(&values) == 1
}

#[derive(Clone, Debug)]
struct Share {
    id: u16,
    ext: u8,
    e: u8,
    group_index: u8,
    group_threshold: usize,
    group_count: usize,
    member_index: u8,
    member_threshold: usize,
    value: Vec<u8>,
}

fn encode_share(s: &Share) -> Result<String> {
    let mut bits = Bits(Vec::new());
    bits.push(u32::from(s.id), 15);
    bits.push(u32::from(s.ext), 1);
    bits.push(u32::from(s.e), 4);
    bits.push(u32::from(s.group_index), 4);
    bits.push((s.group_threshold - 1) as u32, 4);
    bits.push((s.group_count - 1) as u32, 4);
    bits.push(u32::from(s.member_index), 4);
    bits.push((s.member_threshold - 1) as u32, 4);
    bits.push_share_value(&s.value);
    let data = bits.words();
    let cs = customization(s.ext);
    let sum = checksum_words(cs, &data);
    let list = words();
    if list.len() != RADIX {
        bail!("SLIP-39 wordlist has {} words, not {RADIX}", list.len());
    }
    let mut all = data;
    all.extend_from_slice(&sum);
    Ok(all
        .iter()
        .map(|i| list[*i as usize])
        .collect::<Vec<_>>()
        .join(" "))
}

fn decode_share(mnemonic: &str) -> Result<Share> {
    let list = words();
    let idxs: Vec<u16> = mnemonic
        .split_whitespace()
        .map(word_index)
        .collect::<Result<_>>()?;
    if idxs.len() < 20 {
        bail!("a share mnemonic is too short");
    }
    // Try both customization strings; the checksum decides.
    let (ext, data) = [0u8, 1]
        .into_iter()
        .find_map(|ext| {
            let cs = customization(ext);
            if checksum_ok(cs, &idxs) {
                Some((ext, idxs.clone()))
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("share checksum does not match"))?;
    let body = &data[..data.len() - 3];
    let mut raw = Vec::new();
    for w in body {
        for i in (0..10).rev() {
            raw.push(((w >> i) & 1) as u8);
        }
    }
    if raw.len() < 40 {
        bail!("share is missing its header");
    }
    let mut at = 0;
    let take = |at: &mut usize, n: usize| -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | u32::from(raw[*at]);
            *at += 1;
        }
        v
    };
    let id = take(&mut at, 15) as u16;
    let ext_bits = take(&mut at, 1) as u8;
    if ext_bits != ext {
        bail!("share flag does not match its checksum");
    }
    let e = take(&mut at, 4) as u8;
    let group_index = take(&mut at, 4) as u8;
    let group_threshold = take(&mut at, 4) as usize + 1;
    let group_count = take(&mut at, 4) as usize + 1;
    let member_index = take(&mut at, 4) as u8;
    let member_threshold = take(&mut at, 4) as usize + 1;
    let ps = &raw[at..];
    let pad = ps.len() % 16;
    if pad > 8 {
        bail!("share padding is too long");
    }
    if ps.iter().take(pad).any(|b| *b != 0) {
        bail!("share padding is not zero");
    }
    let value_bits = &ps[pad..];
    if value_bits.len() < 128 || value_bits.len() % 8 != 0 {
        bail!("share value has a bad length");
    }
    let value: Vec<u8> = value_bits
        .chunks(8)
        .map(|c| c.iter().fold(0u8, |a, b| (a << 1) | b))
        .collect();
    let _ = list;
    Ok(Share {
        id,
        ext,
        e,
        group_index,
        group_threshold,
        group_count,
        member_index,
        member_threshold,
        value,
    })
}

/// Recover the master secret from share mnemonics and a SLIP-39 passphrase.
/// An empty passphrase is what this program writes. The official vectors use `TREZOR`.
pub fn combine(mnemonics: &[&str], passphrase: &str) -> Result<Zeroizing<Vec<u8>>> {
    if mnemonics.is_empty() {
        bail!("no shares");
    }
    let shares: Vec<Share> = mnemonics
        .iter()
        .map(|m| decode_share(m))
        .collect::<Result<_>>()?;
    let first = &shares[0];
    for s in &shares {
        if s.id != first.id
            || s.ext != first.ext
            || s.e != first.e
            || s.group_threshold != first.group_threshold
            || s.group_count != first.group_count
            || s.value.len() != first.value.len()
        {
            bail!("these shares are not from the same set");
        }
    }
    if first.group_count < first.group_threshold {
        bail!("the group threshold is higher than the group count");
    }
    let mut groups: Vec<u8> = shares.iter().map(|s| s.group_index).collect();
    groups.sort_unstable();
    groups.dedup();
    if groups.len() != first.group_threshold {
        bail!(
            "need {} groups, got {}",
            first.group_threshold,
            groups.len()
        );
    }
    let mut group_secrets: Vec<(u8, Vec<u8>)> = Vec::new();
    for gi in groups {
        let members: Vec<&Share> = shares.iter().filter(|s| s.group_index == gi).collect();
        let threshold = members[0].member_threshold;
        if members.iter().any(|s| s.member_threshold != threshold) {
            bail!("member thresholds in one group do not match");
        }
        let mut idxs: Vec<u8> = members.iter().map(|s| s.member_index).collect();
        let n = idxs.len();
        idxs.sort_unstable();
        idxs.dedup();
        if idxs.len() != n {
            bail!("a member index is repeated");
        }
        if n != threshold {
            bail!("need {threshold} shares in this group, got {n}");
        }
        let parts: Vec<(u8, &[u8])> = members
            .iter()
            .map(|s| (s.member_index, s.value.as_slice()))
            .collect();
        let secret = recover_secret(threshold, &parts)?;
        group_secrets.push((gi, secret));
    }
    let parts: Vec<(u8, &[u8])> = group_secrets
        .iter()
        .map(|(i, v)| (*i, v.as_slice()))
        .collect();
    let ems = recover_secret(first.group_threshold, &parts)?;
    let ms = decrypt_master(&ems, passphrase, first.e, first.id, first.ext);
    Ok(Zeroizing::new(ms))
}

/// One SLIP-39 group. The plan's default is a single group.
pub const GROUP_COUNT: usize = 1;
/// Groups required. With one group, this is 1.
pub const GROUP_THRESHOLD: usize = 1;
/// Shares in the group.
pub const MEMBER_COUNT: usize = 5;
/// Shares required to rebuild the seed.
pub const MEMBER_THRESHOLD: usize = 3;

/// Split a 256-bit seed into 5 SLIP-39 mnemonics, any 3 of which restore it.
/// The share passphrase is empty. The account password stays outside the shares.
/// Each split draws a new identifier and new randomness, so the same seed does
/// not produce the same shares again.
pub fn split_seed(seed: &[u8; 32]) -> Result<Vec<String>> {
    let mut id_bytes = [0u8; 2];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut id_bytes);
    let id = u16::from_be_bytes(id_bytes) & 0x7fff;
    let ext = 1u8;
    let e = 0u8;
    let ems = encrypt_master(seed, "", e, id, ext);
    let group = split_secret(GROUP_THRESHOLD, GROUP_COUNT, &ems)?;
    let members = split_secret(MEMBER_THRESHOLD, MEMBER_COUNT, &group[0])?;
    let mut out = Vec::with_capacity(5);
    for (index, value) in members.into_iter().enumerate() {
        let share = Share {
            id,
            ext,
            e,
            group_index: 0,
            group_threshold: GROUP_THRESHOLD,
            group_count: GROUP_COUNT,
            member_index: index as u8,
            member_threshold: MEMBER_THRESHOLD,
            value,
        };
        out.push(encode_share(&share)?);
    }
    Ok(out)
}

/// The 15-bit set id carried in a share, so a sheet can name its set.
pub fn set_id(mnemonic: &str) -> Result<u16> {
    Ok(decode_share(mnemonic)?.id)
}

/// Member index, counting from 1, and the member count (5 for a seed split here).
pub fn member_position(mnemonic: &str) -> Result<(u8, u8)> {
    let s = decode_share(mnemonic)?;
    Ok((s.member_index + 1, MEMBER_COUNT as u8))
}

/// Pull share mnemonics out of a sheet, a file, or a paste.
/// A share this program issues is 33 words. A 20-word share is accepted so the
/// official 128-bit SLIP-39 vectors still parse. This program does not issue those.
pub fn extract_mnemonics(text: &str) -> Vec<String> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let mut taken = None;
        for len in [33usize, 20] {
            if i + len <= tokens.len() {
                let candidate = tokens[i..i + len].join(" ");
                if decode_share(&candidate).is_ok() {
                    taken = Some(len);
                    out.push(candidate);
                    break;
                }
            }
        }
        i += taken.unwrap_or(1);
    }
    out
}

/// Printable sheet. The password is not on it, and the text says so.
pub fn sheet(acp_id: &str, mnemonic: &str) -> Result<String> {
    let id = set_id(mnemonic)?;
    let (index, of) = member_position(mnemonic)?;
    Ok(format!(
        "ACP recovery share {index} of {of}\n\
         Each share is 33 words. Any 3 of 5 rebuild the same 256-bit secret. The phrase is 24 words.\n\
         acp-id: {acp_id}\n\
         set: {id}\n\
         Reissuing shares of this seed does not invalidate older shares. Only a master rotation does.\n\
         The password is not in this share. The share alone cannot open the account.\n\
         \n\
         {mnemonic}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_vectors() {
        let raw = include_str!("testdata/slip39-vectors.json");
        let vectors: Vec<(String, Vec<String>, String, String)> =
            serde_json::from_str(raw).unwrap();
        for (desc, mnemonics, secret, _xprv) in vectors {
            let refs: Vec<&str> = mnemonics.iter().map(String::as_str).collect();
            let got = combine(&refs, "TREZOR");
            if secret.is_empty() {
                assert!(
                    got.is_err(),
                    "{desc} should fail, got {}",
                    got.unwrap().len()
                );
            } else {
                let got = got.unwrap_or_else(|e| panic!("{desc}: {e}"));
                assert_eq!(data_encoding::HEXLOWER.encode(&got), secret, "{desc}");
            }
        }
    }

    #[test]
    fn three_of_five_every_combination_and_pairs_fail() {
        let seed = [0x5au8; 32];
        let shares = split_seed(&seed).unwrap();
        assert_eq!(shares.len(), 5);
        let mut combos = 0;
        for a in 0..5 {
            for b in a + 1..5 {
                let pair = [shares[a].as_str(), shares[b].as_str()];
                assert!(combine(&pair, "").is_err(), "two shares must fail");
                for c in b + 1..5 {
                    let three = [shares[a].as_str(), shares[b].as_str(), shares[c].as_str()];
                    let got = combine(&three, "").unwrap();
                    assert_eq!(got.as_slice(), &seed);
                    combos += 1;
                }
            }
        }
        assert_eq!(combos, 10);
        // A non-empty SLIP-39 passphrase is not what we stored, so it must not yield the seed.
        let three = [shares[0].as_str(), shares[1].as_str(), shares[2].as_str()];
        let wrong = combine(&three, "TREZOR").unwrap();
        assert_ne!(wrong.as_slice(), &seed);
    }

    #[test]
    fn reissuing_shares_leaves_the_old_set_valid() {
        let seed = [0xa5u8; 32];
        let first = split_seed(&seed).unwrap();
        let second = split_seed(&seed).unwrap();
        let old = [first[0].as_str(), first[2].as_str(), first[4].as_str()];
        let new = [second[1].as_str(), second[3].as_str(), second[4].as_str()];
        assert_eq!(combine(&old, "").unwrap().as_slice(), &seed);
        assert_eq!(combine(&new, "").unwrap().as_slice(), &seed);
        let mixed = [first[0].as_str(), first[1].as_str(), second[2].as_str()];
        assert!(combine(&mixed, "").is_err());
    }

    #[test]
    fn a_sheet_round_trips_and_names_the_account() {
        let seed = [0x11u8; 32];
        let shares = split_seed(&seed).unwrap();
        let text = sheet("acp1-example", &shares[2]).unwrap();
        assert!(text.contains("acp-id: acp1-example"));
        assert!(text.contains("does not invalidate older shares"));
        assert!(text.contains("password is not"));
        assert!(text.contains("33 words"));
        assert!(text.contains("24 words"));
        assert_eq!(shares[2].split_whitespace().count(), 33);
        let found = extract_mnemonics(&text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], shares[2]);
    }
}
