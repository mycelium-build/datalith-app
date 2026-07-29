use std::collections::HashMap;
use std::rc::Rc;

use gpui::*;
use gpui_component::input::{InputEvent, InputState};
use gpui_component::select::SelectState;
use gpui_component::VirtualListScrollHandle;

use crate::document::handler::ReloadOutcome;
use crate::document::todo_txt::{FocusTarget, TodoTxtWorkspace, parse_date};

use super::constants::*;

pub(crate) struct TodoTxtState {
    pub(super) workspace: TodoTxtWorkspace,
    pub(super) priority_picker_open: Option<usize>,
    pub(super) pending_focus_desc: Option<usize>,
    pub(super) pending_focus_search: bool,
    pub(super) search_input: Entity<InputState>,
    pub(super) filter_select: Entity<SelectState<Vec<String>>>,
    pub(super) sort_select: Entity<SelectState<Vec<String>>>,
    pub(super) new_task_input: Entity<InputState>,
    pub(super) desc_inputs: HashMap<usize, Entity<InputState>>,
    pub(super) date_inputs: HashMap<usize, Entity<InputState>>,
    pub(super) desc_subs: HashMap<usize, Subscription>,
    pub(super) date_subs: HashMap<usize, Subscription>,
    pub(super) scroll_handle: VirtualListScrollHandle,
    pub(super) item_sizes: Rc<Vec<Size<Pixels>>>,
    pub(super) _subscriptions: Vec<Subscription>,
}

impl EventEmitter<()> for TodoTxtState {}

impl Focusable for TodoTxtState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search_input.focus_handle(cx)
    }
}

impl TodoTxtState {
    pub(super) fn reload_from_disk(&mut self, cx: &mut Context<Self>) -> anyhow::Result<ReloadOutcome> {
        let outcome = self.workspace.reload_from_disk()?;
        if outcome == ReloadOutcome::Reloaded {
            self.clear_row_inputs();
            self.refresh_item_sizes();
            cx.notify();
        }
        Ok(outcome)
    }

    pub(super) fn add_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.new_task_input.read(cx).value();
        if !value.trim().is_empty() {
            self.workspace.add_task(&value);
            self.clear_row_inputs();
            self.refresh_item_sizes();
            cx.notify();
        }
        self.new_task_input
            .update(cx, |state, cx| state.set_value("", window, cx));
    }

    pub(super) fn refresh_item_sizes(&mut self) {
        // necessary because row count change and v_virtual_list GPUI components need a size for each rows (that can be different)
        let tasks = self.workspace.visible_tasks();
        self.item_sizes = Rc::new(
            tasks
                .iter()
                .map(|_| Size {
                    width: px(0.0),
                    height: px(TODO_ROW_HEIGHT),
                })
                .collect(),
        );
    }

    pub(super) fn toggle_complete(&mut self, index: usize, cx: &mut Context<Self>) {
        self.workspace.toggle_complete(index);
        self.refresh_item_sizes();
        cx.notify();
    }

    pub(super) fn toggle_expand(&mut self, index: usize, cx: &mut Context<Self>) {
        self.workspace.toggle_expanded(index);
        self.refresh_item_sizes();
        cx.notify();
    }

    pub(super) fn add_subtask(&mut self, parent_index: usize, cx: &mut Context<Self>) {
        let outcome = self.workspace.add_subtask(parent_index);
        if let Some(FocusTarget::Task(index)) = outcome.focus {
            self.clear_row_inputs();
            self.pending_focus_desc = Some(index);
        }
        self.refresh_item_sizes();
        cx.notify();
    }

    pub(super) fn delete_task(&mut self, index: usize, cx: &mut Context<Self>) {
        let outcome = self.workspace.delete_task(index);
        self.clear_row_inputs();
        match outcome.focus {
            Some(FocusTarget::Task(index)) => self.pending_focus_desc = Some(index),
            Some(FocusTarget::Search) => self.pending_focus_search = true,
            None => {}
        }
        self.refresh_item_sizes();
        cx.notify();
    }

    fn clear_row_inputs(&mut self) {
        self.desc_inputs.clear();
        self.date_inputs.clear();
        self.desc_subs.clear();
        self.date_subs.clear();
    }

    pub(super) fn commit_description(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        self.workspace.update_description(index, value);
        self.refresh_after_update(cx);
    }

    pub(super) fn commit_date(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        self.workspace.update_date(index, value);
        self.refresh_after_update(cx);
    }

    pub(super) fn commit_priority(&mut self, index: usize, value: &str, cx: &mut Context<Self>) {
        self.workspace.update_priority(index, value);
        self.refresh_after_update(cx);
    }

    fn refresh_after_update(&mut self, cx: &mut Context<Self>) {
        self.priority_picker_open = None;
        self.refresh_item_sizes();
        cx.notify();
    }

    pub(super) fn ensure_row_inputs(
        &mut self,
        flat_index: usize,
        desc: &str,
        date_str: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.desc_inputs.contains_key(&flat_index) {
            let entity = cx.new(|cx| InputState::new(window, cx).default_value(desc.to_string()));
            let idx = flat_index;
            let sub = cx.subscribe_in(
                &entity,
                window,
                move |this, input, event: &InputEvent, _window, cx| {
                    if let InputEvent::Change = event {
                        let value = input.read(cx).value().to_string();
                        this.commit_description(idx, &value, cx);
                    }
                },
            );
            self.desc_subs.insert(flat_index, sub);
            self.desc_inputs.insert(flat_index, entity);
        }

        if !self.date_inputs.contains_key(&flat_index) {
            let entity = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(date_str.to_string())
                    .placeholder("yyyy-mm-dd")
            });
            let idx = flat_index;
            let sub = cx.subscribe_in(
                &entity,
                window,
                move |this, input, event: &InputEvent, _window, cx| {
                    if let InputEvent::Change = event {
                        let value = input.read(cx).value().to_string();
                        if value.is_empty() || parse_date(&value).is_some() {
                            this.commit_date(idx, &value, cx);
                        }
                    }
                },
            );
            self.date_subs.insert(flat_index, sub);
            self.date_inputs.insert(flat_index, entity);
        }
    }
}
