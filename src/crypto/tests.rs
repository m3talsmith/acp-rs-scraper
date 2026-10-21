//! Known answers for the phrase and the master derivation.

use super::*;
use master::{derive_master, normalize_phrase, phrase_from_entropy, recover_seed};
use password::{Argon2Params, MIN_PASSWORD_CHARS, check_password, default_params};

const ABANDON: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
const LEGAL: &str = "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title";
const ZOO: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote";

#[test]
fn bip39_official_24_word_vectors() {
    let cases = [([0u8; 32], ABANDON), ([0x7f; 32], LEGAL), ([0xff; 32], ZOO)];
    for (entropy, phrase) in cases {
        assert_eq!(phrase_from_entropy(&entropy).unwrap(), phrase);
        let back = recover_seed(phrase).unwrap();
        assert_eq!(back.as_slice(), &entropy);
    }
}

#[test]
fn phrase_whitespace_and_case_do_not_change_the_seed() {
    let messy = format!("  {}\n", ABANDON.to_ascii_uppercase());
    assert_eq!(recover_seed(&messy).unwrap().as_slice(), &[0u8; 32]);
    assert_eq!(normalize_phrase("  Zoo   ZOO "), "zoo zoo");
}

#[test]
fn short_or_weak_passwords_are_refused() {
    assert_eq!(MIN_PASSWORD_CHARS, 8);
    let seven = check_password("1234567").unwrap_err().to_string();
    assert!(seven.contains("at least 8 characters"), "{seven}");
    // Eight characters clears the length check. This zxcvbn build scores every
    // 8-character password at most 2 (10^8 + 1 guesses), so the strength check
    // still refuses it. Nine characters of the same shape passes both.
    let eight = check_password("k3$mQ9vL").unwrap_err().to_string();
    assert!(
        !eight.contains("at least 8"),
        "an 8-character password must not be rejected for length: {eight}"
    );
    assert!(eight.contains("too easy"), "{eight}");
    let weak = check_password("passwordpassword").unwrap_err().to_string();
    assert!(weak.contains("too easy"), "{weak}");
    assert!(check_password("k3$mQ9vLx").is_ok());
    assert!(check_password("correct horse battery staple").is_ok());
}

#[test]
fn default_argon2_meets_the_floor_and_the_test_params_do_not() {
    assert!(default_params().meets_floor());
    assert_eq!(default_params(), Argon2Params::PRODUCTION);
    assert!(!Argon2Params::FAST.meets_floor());
}

#[test]
fn wrong_password_derives_a_different_acp_id_and_the_error_shows_it() {
    let phrase = ABANDON;
    let password = "correct horse battery staple";
    let good = derive_master(phrase, password, Argon2Params::FAST).unwrap();
    let bad = derive_master(phrase, "correct horse battery staple!", Argon2Params::FAST).unwrap();
    assert_ne!(good.acp_id, bad.acp_id);
    let err = bad.require_id(&good.acp_id).unwrap_err().to_string();
    assert!(err.contains(bad.acp_id.as_str()), "{err}");
    assert!(err.contains(good.acp_id.as_str()), "{err}");
    assert!(
        err.contains("wrong password") || err.contains("does not match"),
        "{err}"
    );
    assert!(!err.contains(phrase), "{err}");
    assert!(!err.contains(password), "{err}");
    good.require_id(&good.acp_id).unwrap();
}

/// Locked answer for the all-zero BIP-39 phrase, password `correct horse battery staple`,
/// and [`Argon2Params::FAST`]. Recomputed independently with Python (`argon2-cffi`,
/// `blake3`, HKDF-SHA256, `cryptography` ed25519) against the labels in `master`.
#[test]
fn master_derivation_known_answer() {
    let m = derive_master(ABANDON, "correct horse battery staple", Argon2Params::FAST).unwrap();
    assert_eq!(
        m.acp_id.as_str(),
        "acp1-dgp7sz7d5dgyf4afirblorftbmevhqsnklqxrjofjyavu624lwbesjwd"
    );
}

#[test]
fn acp_id_checksum_rejects_a_changed_character() {
    let m = derive_master(ABANDON, "correct horse battery staple", Argon2Params::FAST).unwrap();
    let mut chars: Vec<char> = m.acp_id.as_str().chars().collect();
    let i = chars.iter().rposition(|c| *c != '-').unwrap();
    chars[i] = if chars[i] == 'a' { 'b' } else { 'a' };
    let bad: String = chars.into_iter().collect();
    assert!(AcpId::parse(&bad).is_err());
    assert_eq!(
        AcpId::parse(&m.acp_id.as_str().to_ascii_uppercase())
            .unwrap()
            .as_str(),
        m.acp_id.as_str()
    );
}

#[test]
fn pin_round_trip_stores_no_phrase_and_is_private() {
    let dir = std::env::temp_dir().join(format!("acp-pin-{}-{}", std::process::id(), line!()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let m = derive_master(ABANDON, "correct horse battery staple", Argon2Params::FAST).unwrap();
    let pin = Pin {
        acp_id: m.acp_id.clone(),
        argon2: Argon2Params::FAST,
        sign_pub: m.sign_pub,
        wrap_pub: m.wrap_pub,
    };
    pin.store(&dir).unwrap();
    let again = Pin::load(&dir).unwrap().unwrap();
    assert_eq!(again, pin);
    assert!(pin.store(&dir).is_err(), "refuses to overwrite");
    let path = Pin::path(&dir);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("abandon"), "{text}");
    assert!(text.contains(m.acp_id.as_str()), "{text}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn three_shares_or_the_phrase_recover_the_seed_and_two_shares_do_not() {
    let seed = [0x3cu8; 32];
    let phrase = phrase_from_entropy(&seed).unwrap();
    let shares = slip39::split_seed(&seed).unwrap();
    let two = format!("{}\n{}", shares[0], shares[1]);
    assert!(cmd::seed_from_material(&two).is_err());
    let three = shares[..3].join("\n");
    assert_eq!(cmd::seed_from_material(&three).unwrap().as_slice(), &seed);
    assert_eq!(cmd::seed_from_material(&phrase).unwrap().as_slice(), &seed);
    let sheet = slip39::sheet("acp1-example", &shares[0]).unwrap();
    assert!(!sheet.contains(&phrase));
    assert!(sheet.contains("password is not"));
}

#[test]
fn recovery_next_create_show_and_cannot_rebuild() {
    let dir = std::env::temp_dir().join(format!("acp-next-{}-{}", std::process::id(), line!()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(cmd::recovery_next(&dir).unwrap(), cmd::RecoveryNext::Quiet);
    cmd::mark_shares_pending(&dir).unwrap();
    assert_eq!(cmd::recovery_next(&dir).unwrap(), cmd::RecoveryNext::Create);
    let phrase = phrase_from_entropy(&[0x11; 32]).unwrap();
    let master = derive_master(&phrase, "correct horse battery staple", Argon2Params::FAST).unwrap();
    Pin { acp_id: master.acp_id.clone(), argon2: Argon2Params::FAST, sign_pub: master.sign_pub, wrap_pub: master.wrap_pub }
        .store(&dir)
        .unwrap();
    assert_eq!(cmd::recovery_next(&dir).unwrap(), cmd::RecoveryNext::CannotRebuild);
    recovery::seal(&dir, "correct horse battery staple", Argon2Params::FAST, &[0x11; 32]).unwrap();
    assert_eq!(cmd::recovery_next(&dir).unwrap(), cmd::RecoveryNext::ShowAgain);
    let seal = std::fs::read(recovery::seal_path(&dir)).unwrap();
    assert!(!String::from_utf8_lossy(&seal).contains(&phrase));
    cmd::clear_shares_pending(&dir).unwrap();
    recovery::clear_next(&dir).unwrap();
    assert_eq!(cmd::recovery_next(&dir).unwrap(), cmd::RecoveryNext::Quiet);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shares_are_confirmed_one_at_a_time_and_not_written() {
    let seed = [0x3cu8; 32];
    let shares = slip39::split_seed(&seed).unwrap();
    assert_eq!(shares.len(), slip39::MEMBER_COUNT);
    let mut answers = shares.clone();
    answers.insert(0, "not the share".into());
    let mut n = 0;
    let mut out = Vec::new();
    cmd::present_shares_with("acp1-example", &shares, true, &mut out, || {
        let line = answers[n].clone();
        n += 1;
        Ok(line)
    })
    .unwrap();
    let text = String::from_utf8(out).unwrap();
    let first = text.find("Share 1 of 5").unwrap();
    let missed = text.find("That share did not match").unwrap();
    let second = text.find("Share 2 of 5").unwrap();
    assert!(first < missed && missed < second, "share 2 was shown before share 1 was confirmed");
    assert!(text.contains("Threshold 3 of 5"));
    assert!(text.contains("Group threshold 1 of 1"));
    assert!(text.contains("Destroy any older shares"));
    assert!(!text.contains("not the share"));
    assert_eq!(n, shares.len() + 1);
    let dir = std::env::temp_dir().join(format!("acp-sheets-{}-{}", std::process::id(), line!()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap_or_default();
        let got = String::from_utf8_lossy(&bytes);
        assert!(!got.contains(&shares[0]), "a share was written to disk");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_postponed_share_set_is_reminded_until_it_is_checked() {
    let dir = std::env::temp_dir().join(format!("acp-shares-{}-{}", std::process::id(), line!()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(cmd::shares_reminder(&dir).is_none());
    cmd::mark_shares_pending(&dir).unwrap();
    let msg = cmd::shares_reminder(&dir).unwrap();
    assert!(msg.contains("--show-recovery"), "{msg}");
    cmd::clear_shares_pending(&dir).unwrap();
    assert!(cmd::shares_reminder(&dir).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
