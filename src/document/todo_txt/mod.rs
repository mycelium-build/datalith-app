mod filter;
mod sort;
mod workspace;

pub use filter::FilterKind;
pub use sort::SortKind;
pub use workspace::TodoTxtWorkspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Task(usize),
    Search,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MutationOutcome {
    pub(crate) focus: Option<FocusTarget>,
}

pub fn parse_date(value: &str) -> Option<time::Date> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    time::Date::from_calendar_date(year, time::Month::try_from(month).ok()?, day).ok()
}

fn today_string() -> String {
    time::OffsetDateTime::now_utc().date().to_string()
}

fn matches_task(task: &txtodo::Task, query: &str) -> bool {
    task.description.to_lowercase().contains(query)
        || task
            .priority
            .as_ref()
            .is_some_and(|p| p.to_string().to_lowercase().contains(query))
        || task
            .creation_date
            .is_some_and(|d| d.to_string().to_lowercase().contains(query))
        || task
            .projects
            .iter()
            .any(|p| p.to_lowercase().contains(query))
        || task
            .contexts
            .iter()
            .any(|c| c.to_lowercase().contains(query))
        || task.extensions.iter().any(|(k, v)| {
            k.to_lowercase().contains(query) || v.to_string().to_lowercase().contains(query)
        })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::string_slice
    )]

    use super::*;

    #[test]
    fn parse_valid_date() {
        assert!(parse_date("2024-01-15").is_some());
    }

    #[test]
    fn parse_invalid_date() {
        assert!(parse_date("not-a-date").is_none());
    }
}
