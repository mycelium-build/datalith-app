---
category: format
---

# Todo.txt

A `.todotxt` file is a plain-text task list in the todo.txt format. Datalith gives it a dedicated editor: filter, sort, complete, and nest tasks.

## A task line

```
(A) 2026-01-01 Ship the docs Vault +project @context due:2026-02-01
```

A line is made of optional parts:

- **Priority** `(A)` – a single letter from `(A)` down to `(Z)`.
- **Completion** `x` – the first character when a task is done. Completion date may follow.
- **Date** `YYYY-MM-DD` – creation date after the priority.
- **Description** – the free text of the task.
- **Project** `+name` and **Context** `@name` – tagging tokens in the description.
- **Extensions** – extra `key:value` pairs such as `due:2026-02-01`.

## Subtasks

Indent a line to make it a subtask of the task above. While editing a task, press **Shift+Enter** to add a subtask below it, and use the disclosure arrow to collapse a parent.

## Done

Marking a task complete prefixes it with `x`. You can filter them out, or sort to push them down.

## Using the editor

- Press **Enter** on a task to toggle it complete.
- Press **Shift+Enter** on a task to add a subtask.
- Press **Shift+Backspace** on a task to delete it.
- Press **↑** and **↓** to move from one task to another.
- **Filter** to view all, active, or completed tasks.
- **Sort** by created date and toggle the direction.
- The inline search box matches descriptions, priorities, dates, projects, contexts, and extensions.

Try it: [[Tour.todotxt]] in this Vault is a real todo.txt file you can edit and complete.
