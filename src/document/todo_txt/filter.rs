use txtodo::{Priority, TaskFilter, TaskFilters};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilterKind {
    All,
    Incomplete,
    Completed,
    PriorityA,
    PriorityB,
    PriorityC,
}

impl FilterKind {
    pub(crate) const ALL: &[Self] = &[
        Self::All,
        Self::Incomplete,
        Self::Completed,
        Self::PriorityA,
        Self::PriorityB,
        Self::PriorityC,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Incomplete => "Incomplete",
            Self::Completed => "Completed",
            Self::PriorityA => "Priority A",
            Self::PriorityB => "Priority B",
            Self::PriorityC => "Priority C",
        }
    }

    pub(crate) fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::All)
    }

    pub(super) fn predicate(self) -> Option<TaskFilter> {
        match self {
            Self::All => None,
            Self::Incomplete => Some(TaskFilters::incomplete()),
            Self::Completed => Some(TaskFilters::completed()),
            Self::PriorityA => Some(TaskFilters::by_priority(Priority('A'))),
            Self::PriorityB => Some(TaskFilters::by_priority(Priority('B'))),
            Self::PriorityC => Some(TaskFilters::by_priority(Priority('C'))),
        }
    }
}
