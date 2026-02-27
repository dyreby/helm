//! Output formatting for CLI display.

use crate::model::{ActionKind, IssueAction, IssueFocus, PullRequestAction, PullRequestFocus, RepositoryFocus};

/// Format an act for human-readable display.
pub(super) fn format_action(act: &ActionKind) -> String {
    match act {
        ActionKind::Commit { sha } => {
            format!("committed ({sha})")
        }
        ActionKind::Push { branch, sha } => {
            format!("pushed to {branch} ({sha})")
        }
        ActionKind::PullRequest { number, action } => {
            let verb = match action {
                PullRequestAction::Create => "created",
                PullRequestAction::Merge => "merged",
                PullRequestAction::Comment => "commented on",
                PullRequestAction::Reply => "replied on",
                PullRequestAction::RequestedReview { .. } => "requested review on",
            };
            format!("{verb} PR #{number}")
        }
        ActionKind::Issue { number, action } => {
            let verb = match action {
                IssueAction::Create => "created",
                IssueAction::Close => "closed",
                IssueAction::Comment => "commented on",
            };
            format!("{verb} issue #{number}")
        }
    }
}

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
    fn format_commit_action_kind() {
        let kind = ActionKind::Commit {
            sha: "abc1234".to_string(),
        };
        assert_eq!(format_action(&kind), "committed (abc1234)");
    }

    #[test]
    fn format_push_action_kind() {
        let kind = ActionKind::Push {
            branch: "main".to_string(),
            sha: "abc1234".to_string(),
        };
        assert_eq!(format_action(&kind), "pushed to main (abc1234)");
    }

    #[test]
    fn format_pr_action_kinds() {
        let cases = [
            (PullRequestAction::Create, "created PR #10"),
            (PullRequestAction::Merge, "merged PR #10"),
            (PullRequestAction::Comment, "commented on PR #10"),
            (PullRequestAction::Reply, "replied on PR #10"),
            (
                PullRequestAction::RequestedReview {
                    reviewers: vec!["alice".to_string()],
                },
                "requested review on PR #10",
            ),
        ];
        for (pr_action, expected) in cases {
            let kind = ActionKind::PullRequest {
                number: 10,
                action: pr_action,
            };
            assert_eq!(format_action(&kind), expected);
        }
    }

    #[test]
    fn format_issue_action_kinds() {
        let cases = [
            (IssueAction::Create, "created issue #5"),
            (IssueAction::Close, "closed issue #5"),
            (IssueAction::Comment, "commented on issue #5"),
        ];
        for (issue_action, expected) in cases {
            let kind = ActionKind::Issue {
                number: 5,
                action: issue_action,
            };
            assert_eq!(format_action(&kind), expected);
        }
    }
}
