//! Back means "return to where I came from". Moving up the folder tree is a
//! separate gesture (the folder header), so the two never share a label.

/// What the touch workspace currently shows in its main pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Folder,
    Document,
    Deleted,
    Page,
    Form,
}

/// Visited places, persisted with the session so Back survives a relaunch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History<T> {
    pub backward: Vec<T>,
    pub forward: Vec<T>,
}

impl<T: PartialEq> History<T> {
    /// Records leaving `previous` for `current`. A new navigation forgets the
    /// places that Forward could have returned to.
    pub fn record(&mut self, previous: Option<T>, current: Option<&T>) {
        if let Some(previous) = previous
            && Some(&previous) != current
        {
            self.backward.push(previous);
            self.forward.clear();
        }
    }

    /// Pops places in the requested direction until `visit` accepts one,
    /// then stores `current` on the opposite stack. Places that can no longer
    /// be visited are dropped.
    pub fn step(
        &mut self,
        forward: bool,
        current: Option<T>,
        mut visit: impl FnMut(&T) -> bool,
    ) -> bool {
        loop {
            let next = if forward {
                self.forward.pop()
            } else {
                self.backward.pop()
            };
            let Some(next) = next else { return false };
            if visit(&next) {
                if let Some(current) = current {
                    if forward {
                        self.backward.push(current);
                    } else {
                        self.forward.push(current);
                    }
                }
                return true;
            }
        }
    }

    pub const fn can_back(&self) -> bool {
        !self.backward.is_empty()
    }

    pub const fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub fn clear(&mut self) {
        self.backward.clear();
        self.forward.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::History;

    #[test]
    fn a_new_navigation_clears_forward() {
        let mut history = History::default();
        history.record(Some("root"), Some(&"note"));
        assert!(history.step(false, Some("note"), |_| true));
        assert_eq!(history.forward, ["note"]);
        history.record(Some("root"), Some(&"other"));
        assert!(!history.can_forward());
        assert_eq!(history.backward, ["root"]);
    }

    #[test]
    fn staying_in_place_is_not_a_step() {
        let mut history = History::default();
        history.record(Some("root"), Some(&"root"));
        assert!(!history.can_back());
    }

    #[test]
    fn back_skips_places_that_disappeared() {
        let mut history = History::default();
        history.record(Some("root"), Some(&"deleted"));
        history.record(Some("deleted"), Some(&"note"));
        let mut visited = Vec::new();
        assert!(history.step(false, Some("note"), |place| {
            visited.push(*place);
            *place != "deleted"
        }));
        assert_eq!(visited, ["deleted", "root"]);
        assert_eq!(history.forward, ["note"]);
        assert!(!history.can_back());
    }

    #[test]
    fn back_then_forward_returns_to_the_same_place() {
        let mut history = History::default();
        history.record(Some("results"), Some(&"note"));
        let mut place = "note";
        assert!(history.step(false, Some(place), |p| {
            place = p;
            true
        }));
        assert_eq!(place, "results");
        assert!(history.step(true, Some(place), |p| {
            place = p;
            true
        }));
        assert_eq!(place, "note");
        assert_eq!(history.backward, ["results"]);
    }
}
