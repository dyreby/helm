//! Output formatting for CLI display.

use crate::model::{IssueFocus, PullRequestFocus, RepositoryFocus};

pub(super) fn format_pr_focuses(focuses: &[PullRequestFocus]) -> String {
    if focuses.is_empty() {
        return "summary".to_string();
    }
    focuses
        .iter()
        .map(|f| match f {
            PullRequestFocus::Summary => "summary",
            PullRequestFocus::Files => "files",
            PullRequestFocus::Checks => "checks",
            PullRequestFocus::Diff => "diff",
            PullRequestFocus::Comments => "comments",
            PullRequestFocus::Reviews => "reviews",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn format_issue_focuses(focuses: &[IssueFocus]) -> String {
    if focuses.is_empty() {
        return "summary".to_string();
    }
    focuses
        .iter()
        .map(|f| match f {
            IssueFocus::Summary => "summary",
            IssueFocus::Comments => "comments",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn format_repo_focuses(focuses: &[RepositoryFocus]) -> String {
    if focuses.is_empty() {
        return "issues, pull requests".to_string();
    }
    focuses
        .iter()
        .map(|f| match f {
            RepositoryFocus::Issues => "issues",
            RepositoryFocus::PullRequests => "pull requests",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_empty_pr_focuses_defaults_to_summary() {
        assert_eq!(format_pr_focuses(&[]), "summary");
    }

    #[test]
    fn format_pr_focuses_lists_all() {
        let focuses = vec![
            PullRequestFocus::Summary,
            PullRequestFocus::Diff,
            PullRequestFocus::Reviews,
        ];
        assert_eq!(format_pr_focuses(&focuses), "summary, diff, reviews");
    }

    #[test]
    fn format_empty_issue_focuses_defaults_to_summary() {
        assert_eq!(format_issue_focuses(&[]), "summary");
    }

    #[test]
    fn format_empty_repo_focuses_defaults_to_both() {
        assert_eq!(format_repo_focuses(&[]), "issues, pull requests");
    }
}
