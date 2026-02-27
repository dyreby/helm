//! Output formatting for CLI display.

use crate::model::PullRequestFocus;

pub(super) fn format_pr_focus(focus: &PullRequestFocus) -> &'static str {
    match focus {
        PullRequestFocus::Summary => "summary",
        PullRequestFocus::Full => "full",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_pr_focus_summary() {
        assert_eq!(format_pr_focus(&PullRequestFocus::Summary), "summary");
    }

    #[test]
    fn format_pr_focus_full() {
        assert_eq!(format_pr_focus(&PullRequestFocus::Full), "full");
    }
}
