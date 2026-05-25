---
name: warpctrl-mutating-commands
description: Guides safe use of mutating warpctrl commands for running local Warp app instances. Use when an Agent workflow needs to change app state, settings, terminal input, files, or Warp Drive objects through warpctrl.
---

# warpctrl Mutating Command Recipes

Use this skill when a task explicitly asks you to change a running local Warp app through the provisional `warpctrl` CLI.

If the task only needs to inspect state, use the `warpctrl-read-only` skill instead. Do not mix read-only discovery recipes with mutations unless the mutation is explicitly requested and authorized.

## Ground rules
- Use only commands present in the integrated CLI/protocol contract and advertised by the selected app's action metadata.
- Do not treat parser support as proof that the running app has a live handler. If `implementation_status` is `stub` or a request returns `unsupported_action`, stop and report that the handler is not implemented in the selected app.
- Prefer `--output-format json` for Agent workflows so returned IDs, permission categories, and structured errors can be parsed reliably.
- Always select an explicit `instance_id` before mutating. Avoid implicit active-instance targeting in scripts and Agent workflows.
- Re-read target state immediately before a mutation that depends on a window, tab, pane, session, file, or Drive object ID.
- Never fall back to internal dispatch, GUI automation, direct file edits, or unaudited protocol calls when `warpctrl` denies or does not implement an action.

## Permission categories
`warpctrl` action metadata uses these five local-control permission categories:
- `read_metadata`: app structure and non-sensitive configuration reads.
- `read_underlying_data`: terminal content, command history, input buffers, Drive object content, and other user data reads.
- `mutate_app_state`: visible Warp app state changes such as focusing windows, creating or activating tabs, moving panes, opening surfaces, and file open intents.
- `mutate_metadata_configuration`: allowlisted settings, theme, font, zoom, and appearance writes.
- `mutate_underlying_data`: terminal input injection, command execution, file writes/deletes, Drive CRUD, Drive insertion, and workflow execution.

App-state mutation permission must never be treated as permission to mutate underlying data. Terminal command execution, file writes/deletes, Drive CRUD, Drive insertion, and workflow execution require `mutate_underlying_data` and their own authenticated-user/policy checks.

## Preflight before any mutation
1. Discover compatible instances:
   ```bash
   warpctrl --output-format json instance list
   ```
2. Choose an `instance_id` and pass `--instance <instance_id>` on every follow-up command.
3. Inspect the action metadata:
   ```bash
   warpctrl --output-format json action get --instance <instance_id> <action.name>
   ```
4. Confirm:
   - `implementation_status` is `implemented`;
   - `permission_category` matches the expected category below;
   - `requires_authenticated_user` is satisfied when true;
   - `allowed_invocation_contexts` includes the current context;
   - the task has authorized the exact mutation class.
5. Resolve target IDs with read-only commands, then mutate the selected target. If the target is stale, ambiguous, or missing, stop instead of retrying against a different active target.

## Approval guidance
For Agent workflows, the user's task can authorize low-risk visible app-state changes when it names the desired mutation, such as creating a tab or focusing a pane. For higher-risk mutations, require explicit authorization in the task or stop and ask for approval.

Require explicit approval for:
- destructive app-state changes such as `window.close`, `tab.close`, and `pane.close`;
- all `mutate_underlying_data` actions;
- changes that execute, insert, replace, or clear terminal input;
- file writes or deletes;
- Drive create/update/delete/run/insert actions;
- any mutation where the target or effect is ambiguous.

Approval for `mutate_underlying_data` must identify the operation, target instance/target object, and exact content or command when applicable. Do not infer approval for command execution or file/Drive mutation from a general request to "use warpctrl" or from an app-state mutation grant.

Treat command text, file contents, terminal input, and Drive object content as sensitive. Do not include them in summaries or logs unless necessary for the task and already visible to the user.

## Mutating command groups
Use `action get` before each command group and rely on live action metadata rather than this static list for final implementation status.

### App-state mutations (`mutate_app_state`)
Visible app changes that do not write settings, inject terminal input, execute commands, or mutate files/Drive data.

`tab.create` is the first app-state mutation smoke test and may be live on integrated builds:
```bash
warpctrl --output-format json tab create --instance <instance_id>
```

Other app-state mutation contract entries must be treated as unavailable until their handler shards are integrated and metadata reports `implemented`:
```bash
warpctrl --output-format json app focus --instance <instance_id>
warpctrl --output-format json app settings-open --instance <instance_id>
warpctrl --output-format json app command-palette-open --instance <instance_id> --query "<query>"
warpctrl --output-format json app command-search-open --instance <instance_id> --query "<query>"
warpctrl --output-format json app warp-drive-open --instance <instance_id>
warpctrl --output-format json app warp-drive-toggle --instance <instance_id>
warpctrl --output-format json app resource-center-toggle --instance <instance_id>
warpctrl --output-format json app ai-assistant-toggle --instance <instance_id>
warpctrl --output-format json app code-review-toggle --instance <instance_id>
warpctrl --output-format json app vertical-tabs-toggle --instance <instance_id>
warpctrl --output-format json window create --instance <instance_id>
warpctrl --output-format json window focus --instance <instance_id>
warpctrl --output-format json window close --instance <instance_id>
warpctrl --output-format json tab activate --instance <instance_id>
warpctrl --output-format json tab previous --instance <instance_id>
warpctrl --output-format json tab next --instance <instance_id>
warpctrl --output-format json tab last --instance <instance_id>
warpctrl --output-format json tab move --instance <instance_id> --direction right
warpctrl --output-format json tab close --instance <instance_id>
warpctrl --output-format json pane split --instance <instance_id> --direction right
warpctrl --output-format json pane focus --instance <instance_id>
warpctrl --output-format json pane navigate --instance <instance_id> --direction right
warpctrl --output-format json pane close --instance <instance_id>
warpctrl --output-format json pane maximize --instance <instance_id> --enabled true
warpctrl --output-format json pane resize --instance <instance_id> --direction right --amount 5
warpctrl --output-format json pane previous-session --instance <instance_id>
warpctrl --output-format json pane next-session --instance <instance_id>
warpctrl --output-format json file open --instance <instance_id> /path/to/file --line 10
```

Closing windows, tabs, or panes is tier 4 destructive even though it uses `mutate_app_state`; require explicit approval before running it.

### Metadata/configuration mutations (`mutate_metadata_configuration`)
These write allowlisted local configuration or appearance state. They must not be used for arbitrary settings keys.
```bash
warpctrl --output-format json theme set --instance <instance_id> "Warp Dark"
warpctrl --output-format json appearance set --instance <instance_id> --follow-system-theme true
warpctrl --output-format json appearance font-size --instance <instance_id> increase
warpctrl --output-format json appearance zoom --instance <instance_id> reset
warpctrl --output-format json setting set --instance <instance_id> <key> <value>
warpctrl --output-format json setting toggle --instance <instance_id> <key>
warpctrl --output-format json tab rename --instance <instance_id> "<title>"
```

Before setting or toggling a key, verify it appears in `setting list` or action-specific allowlist metadata. Reject private, debug-only, unsafe, derived, or unsupported settings.

### Underlying-data mutations (`mutate_underlying_data`)
These can alter terminal input, execute code, write/delete files, or mutate authenticated Warp Drive objects. They require explicit `mutate_underlying_data` permission and approval separate from app-state mutations.
```bash
warpctrl --output-format json input insert --instance <instance_id> "cargo check"
warpctrl --output-format json input replace --instance <instance_id> "cargo test"
warpctrl --output-format json input clear --instance <instance_id>
warpctrl --output-format json input mode --instance <instance_id> terminal
warpctrl --output-format json input run --instance <instance_id> "cargo check"
warpctrl --output-format json file write --instance <instance_id> /path/to/file "<contents>" --create
warpctrl --output-format json file delete --instance <instance_id> /path/to/file
warpctrl --output-format json drive create --instance <instance_id> --type workflow "<name>" "<content>"
warpctrl --output-format json drive update --instance <instance_id> --type workflow <id> "<content>"
warpctrl --output-format json drive delete --instance <instance_id> --type workflow <id>
warpctrl --output-format json drive run --instance <instance_id> --type workflow <id>
warpctrl --output-format json drive insert --instance <instance_id> --type notebook <id>
```

Do not run these commands unattended unless the task clearly pre-authorizes the specific operation and content. If authorization is missing or ambiguous, stop before making the mutation.

## Structured errors to preserve
Handle and report these errors without retrying with broader authority or a different target: `local_control_disabled`, `unauthorized_local_client`, `insufficient_permissions`, `authenticated_user_required`, `authenticated_user_unavailable`, `execution_context_not_allowed`, `unsupported_action`, `not_allowlisted`, `invalid_params`, `invalid_selector`, `missing_target`, `ambiguous_instance`, `stale_target`, and `target_state_conflict`.
