//! Node IDs: `TYPE-DOMAIN-0012`, with an optional `namespace:` prefix and
//! an optional `.CHILD` suffix (for example a scenario: `REQ-AUTH-0012.S1`).

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedId<'a> {
    pub namespace: Option<&'a str>,
    pub base: &'a str,
    pub child: Option<&'a str>,
}

pub fn parse(s: &str) -> Option<ParsedId<'_>> {
    let (namespace, rest) = match s.split_once(':') {
        Some((ns, rest)) => (Some(ns), rest),
        None => (None, s),
    };
    if let Some(ns) = namespace {
        let ok = !ns.is_empty()
            && ns
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '.'));
        if !ok {
            return None;
        }
    }
    let (base, child) = match rest.split_once('.') {
        Some((b, c)) => (b, Some(c)),
        None => (rest, None),
    };
    if !valid_base(base) {
        return None;
    }
    if child.is_some_and(|c| !is_valid_segment(c)) {
        return None;
    }
    Some(ParsedId {
        namespace,
        base,
        child,
    })
}

pub fn is_valid_segment(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn valid_base(b: &str) -> bool {
    let mut parts = b.split('-');
    let first = parts.next().unwrap_or("");
    if !first
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        || !is_valid_segment(first)
    {
        return false;
    }
    let rest: Vec<&str> = parts.collect();
    !rest.is_empty() && rest.iter().all(|p| is_valid_segment(p))
}

pub fn is_valid(s: &str) -> bool {
    parse(s).is_some()
}

/// The ID without a child suffix, for example `REQ-AUTH-0012.S1` gives `REQ-AUTH-0012`.
pub fn base_of(s: &str) -> &str {
    parse(s).map_or(s, |p| p.base)
}

/// Crockford base32 without I, L, O and U, so IDs are easy to read aloud.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A 4 character suffix for IDs that must not collide across parallel
/// branches, built from the low 20 bits of `random`. About one million values.
pub fn random_suffix(random: u32) -> String {
    (0..4)
        .map(|i| ALPHABET[((random >> (5 * i)) & 31) as usize] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_ids() {
        for id in [
            "REQ-AUTH-0012",
            "ADR-0007",
            "DEC-2026-0153",
            "C4-orders-api",
            "REQ-AUTH-0012.S1",
            "product/checkout:REQ-CHK-0004.S2",
        ] {
            assert!(is_valid(id), "{id} should be valid");
        }
    }

    #[test]
    fn rejects_bad_ids() {
        for id in [
            "",
            "REQ",
            "-REQ-1",
            "REQ--1",
            "REQ-AUTH-0012.",
            "1REQ-1",
            "a b-1",
            ":REQ-1",
        ] {
            assert!(!is_valid(id), "{id:?} should be invalid");
        }
    }

    #[test]
    fn splits_parts() {
        let p = parse("product/checkout:REQ-CHK-0004.S2").unwrap();
        assert_eq!(p.namespace, Some("product/checkout"));
        assert_eq!(p.base, "REQ-CHK-0004");
        assert_eq!(p.child, Some("S2"));
        assert_eq!(base_of("REQ-AUTH-0012.S1"), "REQ-AUTH-0012");
    }

    #[test]
    fn random_suffixes_are_short_readable_and_valid() {
        assert_eq!(random_suffix(0), "0000");
        assert_eq!(random_suffix(u32::MAX), "ZZZZ");
        for n in [1u32, 12345, 0xDEAD_BEEF, 987_654_321] {
            let suffix = random_suffix(n);
            assert_eq!(suffix.len(), 4);
            assert!(suffix.bytes().all(|b| ALPHABET.contains(&b)), "{suffix}");
            assert!(is_valid(&format!("REQ-ORD-{suffix}")), "{suffix}");
        }
        assert_ne!(random_suffix(1), random_suffix(2));
    }
}
