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
    if child.is_some_and(|c| !valid_segment(c)) {
        return None;
    }
    Some(ParsedId {
        namespace,
        base,
        child,
    })
}

fn valid_segment(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn valid_base(b: &str) -> bool {
    let mut parts = b.split('-');
    let first = parts.next().unwrap_or("");
    if !first
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        || !valid_segment(first)
    {
        return false;
    }
    let rest: Vec<&str> = parts.collect();
    !rest.is_empty() && rest.iter().all(|p| valid_segment(p))
}

pub fn is_valid(s: &str) -> bool {
    parse(s).is_some()
}

/// The ID without a child suffix, for example `REQ-AUTH-0012.S1` gives `REQ-AUTH-0012`.
pub fn base_of(s: &str) -> &str {
    parse(s).map_or(s, |p| p.base)
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
}
