---
name: warp-tour
description: Give the user an interactive, hands-on tour of Warp's terminal, coding, agent, knowledge, and navigation features. Use Ask User Question for every tour decision and Warp Control to show each available destination while keeping the main agent conversation visible.
---

# Warp Tour

Guide the user through Warp from the main agent conversation. Use Warp Control for app navigation and Ask User Question for every menu, navigation decision, hands-on checkpoint, help choice, and cleanup confirmation.

Do not ask tour questions as plain text. Do not assume a destination exists, hard-code keyboard shortcuts, submit terminal input, close a pre-existing target, or silently leave temporary state behind.

## Start-up gate

First, resolve the `warpctrl` command. Try each of the following in order, stopping at the first that succeeds:

```sh
warpctrl instance list
```

If `warpctrl` is not found on PATH, check for a local Rust build in the current working directory:

```sh
# Prefer release over debug when both exist
if [ -x ./target/release/warp ]; then
  ./target/release/warp --warpctrl instance list
elif [ -x ./target/debug/warp ]; then
  ./target/debug/warp --warpctrl instance list
fi
```

Record the resolved command prefix as **`$WARPCTRL`** (either `warpctrl`, `./target/release/warp --warpctrl`, or `./target/debug/warp --warpctrl`) and use it for every subsequent command in this skill (e.g. `$WARPCTRL surface list`, `$WARPCTRL tab create`). If none of these work, print:

> Warp Control is required for the guided tour. Enable it in **Settings > Scripting**, then rerun `warp-tour`. If you are developing Warp locally, make sure you have built the project first (`cargo build --features warp_control_cli`).

Ask the user to rerun `warp-tour`, then stop. Do not attempt a fallback tour.

If `$WARPCTRL instance list` succeeds but no compatible instance is found, or local control is disabled, print:

> Warp Control is required for the guided tour. Enable it in **Settings > Scripting**, then rerun `warp-tour`.

Ask the user to rerun `warp-tour`, then stop. Do not attempt a fallback tour.

When more than one instance is running, use Ask User Question to let the user choose one, then pass its exact `--instance` ID to every command.

Query structured state before presenting any topics:

```sh
$WARPCTRL --output-format json app active
$WARPCTRL --output-format json surface list
```

Record the starting window, tab, pane, and session as the **anchor**. The anchor is the main tour agent conversation. Build menus only from surfaces whose `is_available` value is `true`; do not show unavailable topics. If a direct open later returns `unsupported_action` or `target_state_conflict`, explain that the topic is unavailable in the current context, remove it from later menus, and continue.

## Keep the tour agent visible

Keep the anchor visible and return focus to it before every Ask User Question.

1. Prefer one reusable right-hand split for demonstrations:

   ```sh
   $WARPCTRL pane split --direction right --pane <anchor-pane-id>
   $WARPCTRL --output-format json pane list
   ```

   Compare pane lists to identify and record the newly created **tour pane**. Never infer an ID.

2. Target demonstrations at the tour pane with `--pane <tour-pane-id>` when the command accepts a pane selector.
3. Reuse that pane throughout the tour. Do not create a new tab merely to switch topics.
4. If a destination cannot be demonstrated in a split, use a temporary tab only as a fallback, record its returned ID, and return to the anchor immediately after the demonstration.
5. Before every Ask User Question:

   ```sh
   $WARPCTRL pane focus --pane <anchor-pane-id>
   ```

6. If the anchor becomes stale or cannot be focused, stop opening destinations and proceed to cleanup.

## Question flow

Start with Ask User Question:

- **Start the short core tour**
- **Browse the topic menu**
- **End the tour**

For every stop:

1. Open the destination with the narrowest direct Warp Control command.
2. Briefly explain what it is, where it lives, and one useful workflow.
3. Retrieve relevant shortcuts with `$WARPCTRL keybinding get <name>` or `$WARPCTRL keybinding list`; never hard-code a shortcut.
4. Give one small hands-on task.
5. Focus the anchor and use Ask User Question:
   - **I completed it**
   - **I want help**
   - **Skip this task**
   - **End the tour**
6. Wait for the response before restoring temporary state or moving on.
7. After the task is complete or skipped, use Ask User Question:
   - **Continue**
   - **Return to topic menu**
   - **End the tour**

If the user asks for help, explain the task, reopen the same destination if needed, and ask the hands-on checkpoint again. Never repeat a question the user skipped.

## Short core tour

Offer available stops in this order. Allow skip, topic menu, or end after every stop.

### Themes

Save the full result of `$WARPCTRL --output-format json theme get` before opening:

```sh
$WARPCTRL surface theme-picker open --pane <tour-pane-id>
```

Explain previewing and choosing themes. Ask the user to preview a theme, wait for confirmation, then restore the saved theme state:

- Restore saved light and dark themes with `theme light-set` and `theme dark-set`.
- Restore whether system themes were enabled with `theme system-set`.
- If system themes were disabled, restore the saved active theme with `theme set`.

### Keybindings

```sh
$WARPCTRL surface keybindings open --pane <tour-pane-id>
$WARPCTRL keybinding list
```

Explain searching by action or shortcut and customizing a binding. Ask the user to find a command they use often without requiring them to change it.

### Panes and major panels

Explain that panes divide a tab, while tools and code review appear in side panels. Use only available destinations:

```sh
$WARPCTRL surface project-explorer open --pane <tour-pane-id>
$WARPCTRL surface conversation-list open --pane <tour-pane-id>
$WARPCTRL surface warp-drive open --pane <tour-pane-id>
$WARPCTRL surface code-review open --pane <tour-pane-id>
$WARPCTRL pane list
```

Open one destination at a time. Ask the user to identify the anchor pane, tour pane, and currently open panel.

### Global file search

```sh
$WARPCTRL surface global-search open --pane <tour-pane-id>
```

Explain repository-wide file-content search and ask the user to enter a harmless query. Do not submit or modify terminal input on their behalf.

### Vertical tabs

```sh
$WARPCTRL surface vertical-tabs open --pane <tour-pane-id>
$WARPCTRL tab list
$WARPCTRL pane list
$WARPCTRL session list
```

Explain the tab, pane, and session hierarchy and ask the user to locate the tour split in vertical tabs.

After the final available core stop, use Ask User Question to offer the optional topic groups or end.

## Topic menu

Use Ask User Question and omit groups with no available demonstrations:

- **Terminal fundamentals**
- **Coding workflow**
- **Agents**
- **Knowledge and navigation**
- **Resume core tour** when core stops remain
- **End the tour**

After completing a group, return to this menu until the user ends.

### Terminal fundamentals

Cover blocks, modern input, autosuggestions, completions, and command search. Use the reusable terminal split and:

```sh
$WARPCTRL surface command-search open --pane <tour-pane-id>
$WARPCTRL keybinding list
```

Ask the user to run a harmless command themselves, identify its block, try an autosuggestion or completion, and find it in command search. Never invoke a command-submission action.

### Coding workflow

Use available destinations among:

```sh
$WARPCTRL surface project-explorer open --pane <tour-pane-id>
$WARPCTRL surface global-search open --pane <tour-pane-id>
$WARPCTRL surface code-review open --pane <tour-pane-id>
$WARPCTRL file open <user-approved-path>
```

Explain repository opening, project explorer, file editing, global search, and code review. Open a file only after the user chooses or approves its path.

### Agents

Use available destinations among:

```sh
$WARPCTRL surface agent-management open --pane <tour-pane-id>
$WARPCTRL surface conversation-list open --pane <tour-pane-id>
$WARPCTRL surface settings open --query permissions --pane <tour-pane-id>
```

Explain Agent Mode, agent management, permissions, and conversation history. Do not start an agent run or change permissions during the tour.

### Knowledge and navigation

Use available destinations among:

```sh
$WARPCTRL surface warp-drive open --pane <tour-pane-id>
$WARPCTRL surface command-palette open --query "notebook" --pane <tour-pane-id>
$WARPCTRL surface command-palette open --query "workflow" --pane <tour-pane-id>
$WARPCTRL surface settings open --query "MCP" --pane <tour-pane-id>
$WARPCTRL tab list
$WARPCTRL pane list
$WARPCTRL session list
```

Explain Warp Drive, Rules, Notebooks, Workflows, MCP, Command Palette, tabs, panes, and sessions. Ask the user to locate one knowledge item or navigation target; do not create or edit it.

## Cleanup

Run cleanup whenever the user ends, the tour completes, or the anchor becomes unavailable.

1. Restore saved theme state if the theme stop changed it.
2. Focus the anchor if it still exists.
3. List tabs and panes again. Close only the tour pane and fallback tabs whose exact IDs were recorded as tour-created.
4. Before issuing close actions, use Ask User Question to tell the user that Warp may show its normal close warnings:
   - **Clean up tour panes/tabs**
   - **Leave them open and end**
5. If cleanup is chosen, issue exact-ID close commands and tell the user to respond to any normal Warp close warnings:

   ```sh
   $WARPCTRL pane close --pane <tour-pane-id>
   $WARPCTRL tab close --tab <tour-created-tab-id>
   ```

6. Never close the anchor or any target that existed before the tour. If a warning cancels the close or cleanup fails, report exactly what remains open.
