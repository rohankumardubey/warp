---
name: warpctrl-read-only
description: Use read-only warpctrl commands safely to inspect running local Warp app instances, choose explicit targets, and distinguish metadata reads from underlying-data reads. Use when gathering local Warp state through warpctrl for scripts, demos, or Agent workflows.
---

# warpctrl Read-Only Recipes

Use this skill when a task asks you to inspect or reason about a running local Warp app through the provisional `warpctrl` CLI without changing app state.

## Ground rules
- Use only commands present in the integrated read-only CLI contract.
- Do not call mutating commands from this skill. `warpctrl tab create` is implemented as a first-slice app-state mutation smoke test, but it is not read-only.
- Do not treat parser support as proof that the selected app build has a live handler. If a command returns `unsupported_action`, report that the handler is not implemented in the running app and stop that recipe.
- Keep metadata reads separate from underlying-data reads. Metadata read permission does not authorize reading terminal output, input buffers, command history, or object contents.
- Prefer `--output-format json` for Agent workflows so errors and returned IDs can be parsed reliably.

## Select a target safely
1. Discover compatible instances:
   ```bash
   warpctrl --output-format json instance list
   ```
2. Choose an `instance_id` from the result.
3. Pass `--instance <instance_id>` on every follow-up command in scripts or Agent workflows.
4. Use implicit active-instance targeting only for short interactive checks when exactly one compatible instance is present.
5. Avoid `--pid` for durable automation. It is a convenience filter for local debugging, not the canonical selector.

Handle these structured errors explicitly: `no_instance`, `ambiguous_instance`, `local_control_disabled`, `unauthorized_local_client`, `insufficient_permissions`, `authenticated_user_required`, `authenticated_user_unavailable`, `execution_context_not_allowed`, `unsupported_action`, and `stale_target`.

## Metadata read recipes
Metadata reads inspect app structure or local configuration without exposing terminal contents.

### Health and protocol metadata
```bash
warpctrl --output-format json app ping --instance <instance_id>
warpctrl --output-format json app version --instance <instance_id>
```

### Active target chain and app summary
These commands are in the read-only contract. Use them only when the selected app advertises implemented handlers; otherwise expect `unsupported_action`.
```bash
warpctrl --output-format json app active --instance <instance_id>
warpctrl --output-format json app inspect --instance <instance_id>
```

### Action catalog
Use action metadata, when implemented, to confirm `implementation_status`, `permission_category`, and `requires_authenticated_user` before relying on a command.
```bash
warpctrl --output-format json action list --instance <instance_id>
warpctrl --output-format json action get --instance <instance_id> tab.list
```

### Layout and local configuration metadata
```bash
warpctrl --output-format json window list --instance <instance_id>
warpctrl --output-format json tab list --instance <instance_id>
warpctrl --output-format json pane list --instance <instance_id>
warpctrl --output-format json session list --instance <instance_id>
warpctrl --output-format json theme list --instance <instance_id>
warpctrl --output-format json appearance get --instance <instance_id>
warpctrl --output-format json setting list --instance <instance_id>
warpctrl --output-format json setting get --instance <instance_id> appearance.theme
warpctrl --output-format json file list --instance <instance_id>
```

### Warp Drive metadata
`drive list` is a metadata read, but it is authenticated-user data. Use it only when the task requires Drive object names/IDs and the selected app has authenticated-user access.
```bash
warpctrl --output-format json drive list --instance <instance_id>
warpctrl --output-format json drive list --instance <instance_id> --type workflow
```

## Underlying-data read recipes
Underlying-data reads can expose terminal-derived or user-authored content. Use them only when the task requires that content and prefer the narrowest command with a limit.

```bash
warpctrl --output-format json block list --instance <instance_id> --limit 10
warpctrl --output-format json block get --instance <instance_id> <block_id>
warpctrl --output-format json input get --instance <instance_id>
warpctrl --output-format json history list --instance <instance_id> --limit 20
warpctrl --output-format json drive get --instance <instance_id> --type notebook <id>
```

Treat returned block output, current input text, command history, and Drive object content as sensitive task context. Do not include it in summaries unless it is necessary to answer the user's request.

## Commands this skill must not use
Do not use or document these as implemented read-only recipes:
- window, tab, or pane mutations such as create, focus, close, split, activate, move, rename, maximize, resize, or navigate;
- theme, appearance, or setting writes such as set, toggle, font-size, or zoom changes;
- app surface toggles or opens such as settings, command palette, Warp Drive, resource center, AI assistant, code review, or vertical tabs;
- terminal input mutations such as insert, replace, clear, mode switching, or command execution.

If the user explicitly asks for a mutation, leave this skill and verify the command's implemented action metadata and permission category before proceeding.
