---
category: reference
---

Open **Shortcuts** from the gear menu or the Datalith menu, or press `Cmd/Ctrl+/`. It opens in a workspace tab; opening it again returns to the same tab. Use the arrow keys, Page Up / Page Down, Home / End, or the scroll wheel to move through it. Workspace shortcuts still work while the tab is focused.

`Cmd/Ctrl` means **Command** on macOS and **Ctrl** on Linux and Windows. The application shows the platform's notation in the Shortcuts tab. The tables below list the default shortcuts.

# Change shortcuts

- Click an action's shortcut, press the new keys, then click **Save**. Press **Escape** or click **Cancel** to keep the current shortcut. If a key can't be used, such as Escape, Tab or Enter alone, the tab says so and offers **Try again**.
- **Active in** shows where a shortcut works: **Everywhere**, **Sidebar** (while the file tree has focus), **Notes**, **Todo lists** or **Bases**. Click it to choose another place, when the action has more than one.
- While you type, a text field keeps its own keys, such as `Cmd/Ctrl+Backspace` to delete a word. A key that types a character, like `A` or `Shift+A`, never runs a shortcut in a text field. In notes and todo lists, a shortcut needs `Cmd/Ctrl`, `Alt` or `Shift`, or a function key. In the sidebar, a single key such as `N` works.
- While a panel such as Settings or the palette is open, only **Quit**, **Open settings**, **Toggle theme**, **Quick switcher** and **Search files** respond.
- Keys belong to one action per place. Saving keys that another action uses in the same place removes them from that action, which then shows **No shortcut**. An info icon marks a shortcut that another binding takes over somewhere, such as while typing.
- Hover a row to **Reset** it to its default or **Remove** its shortcut. **Reset all** restores every default.
- Changes apply immediately, including in the menus, and are saved in the user `config.json`. If saving fails, the tab says so and the changes apply until you quit.

# File

| Action | Shortcut |
| --- | --- |
| Quit | `Cmd/Ctrl+Q` |
| New note | `Cmd/Ctrl+N` |
| New folder | `Cmd/Ctrl+Shift+N` |
| Rename | `F2` |
| Delete | `Cmd/Ctrl+Backspace` |
| Duplicate | `Cmd/Ctrl+D` |
| Open in Explorer | `Cmd/Ctrl+Shift+E` |
| Copy path | `Cmd/Ctrl+L` |

Rename, Delete, Duplicate, Open in Explorer and Copy path act on the file selected in the sidebar, while the sidebar has focus. Click the open file or an empty area of the sidebar, or press `Cmd/Ctrl+0`, to focus it.

# Navigation

| Action | Shortcut |
| --- | --- |
| Quick switcher | `Cmd/Ctrl+P` |
| Focus sidebar | `Cmd/Ctrl+0` |
| Navigate back | `Cmd/Ctrl+[` |
| Navigate forward | `Cmd/Ctrl+]` |
| Open link | `Cmd/Ctrl+Enter` |

In a text field, `Cmd/Ctrl+[` and `Cmd/Ctrl+]` change indentation. Open link works in notes.

# Wiki links

Type `[[` and three characters to open link suggestions. Use the arrow keys, Enter and Escape to pick or dismiss one.

# Tabs

| Action | Shortcut |
| --- | --- |
| New tab | `Cmd/Ctrl+T` |
| Close tab | `Cmd/Ctrl+W` |
| Select tab | `Cmd/Ctrl+1` … `Cmd/Ctrl+8` |
| Select last tab | `Cmd/Ctrl+9` |

# View

| Action | Shortcut |
| --- | --- |
| Search files | `Cmd/Ctrl+Shift+F` |
| Toggle edit / view | `Cmd/Ctrl+E` |
| Toggle theme | `Cmd/Ctrl+Shift+D` |
| Open settings | `Cmd/Ctrl+,` |

# Todo list

| Action | Shortcut |
| --- | --- |
| Complete task | `↩` |
| Add subtask | `Shift+↩` |
| Delete task | `Cmd/Ctrl+Shift+Backspace` |
| Previous / next task | `↑` / `↓` |
| Search tasks | `Cmd/Ctrl+F` |

On a priority, `↑` and `↓` change it. In a date, Escape restores the saved one.

# Help

| Action | Shortcut |
| --- | --- |
| Show shortcuts | `Cmd/Ctrl+/` |

See [[Basics]] if you are new to the app.
