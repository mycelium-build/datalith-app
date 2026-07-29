use txtodo::{SortDirection, TaskSorter, TaskSorts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortKind {
    Priority,
    DateCreated,
    Description,
    Project,
    Context,
}

impl SortKind {
    pub(crate) const ALL: &[Self] = &[
        Self::Priority,
        Self::DateCreated,
        Self::Description,
        Self::Project,
        Self::Context,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Priority => "Priority",
            Self::DateCreated => "Date",
            Self::Description => "Description",
            Self::Project => "Project",
            Self::Context => "Context",
        }
    }

    pub(crate) fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    pub(super) fn sorter(self, descending: bool) -> TaskSorter {
        let direction = if descending {
            SortDirection::Desc
        } else {
            SortDirection::Asc
        };
        match self {
            Self::Priority => TaskSorts::by_priority(direction),
            Self::DateCreated => TaskSorts::by_date_created(direction),
            Self::Description => TaskSorts::by_description(direction),
            Self::Project => TaskSorts::by_project(direction),
            Self::Context => TaskSorts::by_context(direction),
        }
    }
}
