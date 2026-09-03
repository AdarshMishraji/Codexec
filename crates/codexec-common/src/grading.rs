use crate::models::SubmissionVerdict;

/// Normalizes a string for comparison: trims trailing whitespace on each
/// line, then strips trailing blank lines. Deliberately simple (exact
/// match after normalization) — this is "basic" grading, not a full
/// checker-plugin system.
fn normalize(s: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = s.lines().map(|l| l.trim_end()).collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

pub fn grade(expected: &str, actual: &str) -> SubmissionVerdict {
    if normalize(expected) == normalize(actual) {
        SubmissionVerdict::Accepted
    } else {
        SubmissionVerdict::WrongAnswer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert_eq!(grade("hello\n", "hello\n"), SubmissionVerdict::Accepted);
    }

    #[test]
    fn trailing_whitespace_and_blank_lines_ignored() {
        assert_eq!(grade("hello  \nworld\n\n\n", "hello\nworld"), SubmissionVerdict::Accepted);
    }

    #[test]
    fn mismatch_is_wrong_answer() {
        assert_eq!(grade("hello\n", "goodbye\n"), SubmissionVerdict::WrongAnswer);
    }

    #[test]
    fn internal_whitespace_matters() {
        assert_eq!(grade("hello world", "hello  world"), SubmissionVerdict::WrongAnswer);
    }
}
