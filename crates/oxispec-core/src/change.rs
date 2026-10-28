//! Change sets: a proposed change to the specs, with a lifecycle and a
//! recorded approval. A change set is a node of type `change`.

/// The lifecycle, in order.
pub const STATUSES: &[&str] = &["draft", "in-review", "approved", "merged", "reverted"];

/// The statuses a change set can move to from `from`.
///
/// ```text
/// draft -> in-review -> approved -> merged -> reverted
///              ^            |
///              +------------+   (changes after approval go back to review)
/// ```
pub fn next_statuses(from: &str) -> &'static [&'static str] {
    match from {
        "draft" => &["in-review"],
        "in-review" => &["draft", "approved"],
        "approved" => &["in-review", "merged"],
        "merged" => &["reverted"],
        _ => &[],
    }
}

/// Approved and merged change sets must say who approved them.
pub fn needs_approval(status: &str) -> bool {
    matches!(status, "approved" | "merged")
}

/// Moving back to draft or review withdraws an earlier approval.
pub fn withdraws_approval(to: &str) -> bool {
    matches!(to, "draft" | "in-review")
}

/// A date as `YYYY-MM-DD` in UTC, from seconds since the Unix epoch.
pub fn utc_date(seconds: u64) -> String {
    // Days to civil date, from Howard Hinnant's date algorithms.
    let z = (seconds / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lifecycle_moves_forward_and_back_to_review() {
        assert_eq!(next_statuses("draft"), ["in-review"]);
        assert_eq!(next_statuses("in-review"), ["draft", "approved"]);
        assert_eq!(next_statuses("approved"), ["in-review", "merged"]);
        assert_eq!(next_statuses("merged"), ["reverted"]);
        assert!(next_statuses("reverted").is_empty());
        assert!(next_statuses("nonsense").is_empty());
        for status in STATUSES {
            for next in next_statuses(status) {
                assert!(STATUSES.contains(next), "{next}");
            }
        }
    }

    #[test]
    fn approval_rules() {
        assert!(needs_approval("approved") && needs_approval("merged"));
        assert!(!needs_approval("draft") && !needs_approval("reverted"));
        assert!(withdraws_approval("draft") && withdraws_approval("in-review"));
        assert!(!withdraws_approval("merged"));
    }

    #[test]
    fn dates_are_correct_around_leap_days_and_year_ends() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(86_399), "1970-01-01");
        assert_eq!(utc_date(86_400), "1970-01-02");
        assert_eq!(utc_date(951_782_400), "2000-02-29");
        assert_eq!(utc_date(1_709_164_800), "2024-02-29");
        assert_eq!(utc_date(1_709_251_200), "2024-03-01");
        assert_eq!(utc_date(1_767_225_599), "2025-12-31");
        assert_eq!(utc_date(1_767_225_600), "2026-01-01");
    }
}
