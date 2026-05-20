# Queued Prompts UI — Technical Spec
See `specs/REMOTE-1543/PRODUCT.md` for user-visible behavior. This document covers implementation only.
## Context
`QueueSlashCommand` remains the rollout gate for regular Agent Mode prompt queueing. That includes `/queue`, the auto-queue toggle / button / keybinding, and the related regular prompt enqueueing paths. The new queued prompts panel is the regular prompt queue UI for those rows; it does not have a separate rollout flag.

Cloud Mode prompt placeholders and compact follow-up prompts are compatibility paths, not regular Agent Mode queue rows. Cloud Mode initial / follow-up placeholder handling, `/compact-and`, and `/fork-and-compact` stay on the legacy pending-user-query UI unconditionally for now.
The implementation should therefore be additive and narrowly scoped:
- Keep `QueueSlashCommand` as the only regular prompt queue feature gate.
- Keep the new multi-row queue model and input-adjacent panel focused on regular Agent Mode queued prompts.
- Restore the prior Cloud Mode and compact follow-up pending-query codepaths where practical instead of inventing replacement queue-model logic.
- Remove Cloud Mode / compact-only panel or model accommodations from this workstream so the regular queue architecture stays narrow.
## Feature gating
Do not add `NewQueuedPromptUI`, a `new_queued_prompt_ui` Cargo feature, or any parallel rollout switch for the panel.

`FeatureFlag::QueueSlashCommand` gates the regular Agent Mode queue experience:
- `/queue <prompt>`
- the auto-queue toggle / button / keybinding
- `WorkspaceAction::QueuePromptForConversation`
- related regular prompt enqueueing paths that feed the same Agent Mode queue

When `QueueSlashCommand` is enabled, regular Agent Mode queued prompts append to `QueuedQueryModel` and render in `QueuedPromptsPanelView`. When it is disabled, those regular queue trigger surfaces remain unavailable as they do today. Cloud Mode placeholder handling and compact follow-up placeholders do not branch on this feature gate.
## Rollout behavior
The rollout matrix is:
- `QueueSlashCommand` off: regular Agent Mode queue trigger surfaces stay disabled, and the regular queue panel has no feature-enabled rows to render. Cloud Mode and compact follow-up placeholder flows continue to use the legacy pending-user-query UI.
- `QueueSlashCommand` on: regular Agent Mode queue trigger surfaces append to `QueuedQueryModel`, and `QueuedPromptsPanelView` renders those queued rows. Cloud Mode and compact follow-up placeholder flows still use the legacy pending-user-query UI.

`PendingUserQueryIndicator` and the legacy pending-user-query rich-content path remain compatibility infrastructure for the legacy placeholder flows below. They are not rollout switches for the regular queued prompts panel.
## Regular Agent Mode queue path
Regular Agent Mode queued prompts use the new queue model and panel:
- `QueuedQueryModel` in `app/src/ai/blocklist/queued_query.rs`
- `QueuedPromptsPanelView` in `app/src/ai/blocklist/queued_prompts_panel.rs`
- `Input::queued_prompts_panel` in `app/src/terminal/input.rs`
- `TerminalView::drain_queued_prompts` in `app/src/terminal/view.rs`

`QueuedPromptsPanelView::should_render` must require:
- `FeatureFlag::QueueSlashCommand.is_enabled()`
- an active conversation with regular Agent Mode queued rows

It should not require `PendingUserQueryIndicator`, and it must not depend on a separate queued-prompt UI feature flag.
### `QueuedQueryModel`
`QueuedQueryModel` owns regular Agent Mode prompt queueing:
- `queues: HashMap<AIConversationId, Vec<QueuedQuery>>`
- `editing: Option<EditingRow>`
- `collapsed: HashSet<AIConversationId>`
- `queue_next_prompt_enabled: bool`

The model should only describe regular user-managed queued prompts. Editing, deleting, reordering, collapsing, and queue draining belong here. Cloud Mode placeholder lifecycle and compact follow-up placeholder lifecycle do not.
### Queue trigger routing
Regular prompt trigger surfaces should append to `QueuedQueryModel` with regular queue origins:
- `Input::maybe_queue_input_for_in_progress_conversation` appends `QueuedQueryOrigin::AutoQueueToggle`.
- `/queue <prompt>` in `app/src/terminal/input/slash_commands/mod.rs` appends `QueuedQueryOrigin::QueueSlashCommand` while the selected conversation is in progress; the idle path still submits immediately.
- `WorkspaceAction::QueuePromptForConversation` appends the regular auto-queue origin used by the button / keybinding path.

`/compact-and <prompt>` and `/fork-and-compact <prompt>` are not regular queue trigger surfaces in this workstream. They stay on the legacy pending-user-query UI described below.
## Legacy pending-user-query paths
### Cloud Mode initial and follow-up placeholders
Restore the old pending-query code as-is where practical and keep Cloud Mode initial / follow-up placeholder handling on it unconditionally:
- `app/src/ai/blocklist/block/pending_user_query_block.rs`
- `app/src/terminal/view/pending_user_query.rs`
- `RichContentMetadata::PendingUserQuery` and `RichContent::is_pending_user_query` in `app/src/terminal/view/rich_content.rs`
- `PendingUserQueryKind`, `pending_user_query_view_id`, and `pending_user_query_kind` in `app/src/terminal/view.rs`
- selected-text plumbing for `PendingUserQueryBlock` in `app/src/terminal/model/blocks/selection.rs` and `TerminalView::pending_user_query_selected_text`

Do not restore the legacy single-slot queued prompt callback as the regular Agent Mode queueing implementation. Regular Agent Mode queued prompts belong in `QueuedQueryModel`; Cloud Mode placeholders remain separate rich-content UI.

Legacy Cloud Mode behavior:
- Cloud Mode initial prompt setup and follow-up placeholder handling show the old pending user query block.
- The old Cloud Mode block has no dismiss or send-now affordances; the cloud run lifecycle owns removal.
- When the real shared-session transcript content, auth, cancellation, or non-setup-v2 failure path takes over, the old block is removed by the restored legacy removal helper.
- For `CloudModeSetupV2` failures, keep the old block visible above the failure/tombstone state so the user can still see the prompt that was submitted.
Cloud Mode lifecycle handlers in `app/src/terminal/view/ambient_agent/view_impl.rs` should keep calling the restored legacy insertion / removal helpers. They should not create `QueuedQueryModel` rows or panel-specific Cloud Mode state in this workstream.
### `/compact-and` and `/fork-and-compact`
Restore the prior `/compact-and` and `/fork-and-compact` pending-user-query codepaths where practical:
- `Workspace::summarize_active_ai_conversation`
- `Workspace::handle_forked_conversation_prompts`

These commands should keep their legacy placeholder behavior after starting the summarize / fork-and-summarize work. They should not append `QueuedQueryModel` rows, add panel-only follow-up logic, or invent a new queue-model lifecycle for compact follow-up prompts.
## Out of scope for the regular queue panel / model
Any panel or model behavior added solely to migrate Cloud Mode placeholders or compact follow-up prompts into the new regular queue UI is no longer part of this workstream. Keep it out of the regular queue design rather than preserving it behind hidden conditions:
- no `QueuedQueryOrigin::InitialCloudMode`
- no `QueuedQueryOrigin::CompactAnd`
- no `QueuedQueryOrigin::ForkAndCompact`
- no `AmbientAgentViewModel::cloud_mode_queued_query_id`
- no non-user-managed queue-panel rows or retention / removal rules that only exist for Cloud Mode or compact placeholder migration
## Terminal view wiring
`TerminalView::new` should construct and attach `QueuedPromptsPanelView` as part of the `QueueSlashCommand`-enabled regular queue experience. When `QueueSlashCommand` is disabled, `Input::queued_prompts_panel` stays `None`.

`TerminalView::handle_ai_controller_event` should call `drain_queued_prompts(conversation_id, finish_reason, ctx)` for regular Agent Mode queued prompts. That drain is model-owned behavior, not panel-owned behavior, and it is unrelated to legacy pending-user-query placeholders.

When an active AI block is detected for a different conversation, keep the restored legacy guard only for the pending-user-query placeholder path so stale Cloud Mode placeholder UI is cleared correctly. Regular Agent Mode queued prompts should rely on `QueuedQueryModel` conversation scoping.
## Rich content and selection
Because the legacy pending-user-query UI remains for Cloud Mode placeholders and compact follow-up prompts, restore the rich-content metadata and selection support:
- `RichContentMetadata::PendingUserQuery { pending_user_query_block_handle }`
- `RichContent::is_pending_user_query`
- `read_selected_text_from_pending_user_query_block`
- `TerminalView::pending_user_query_selected_text`
This code should be used by the legacy placeholder paths only, but it can remain compiled unconditionally to minimize churn and keep the restored code close to the old implementation.
## Telemetry
New panel-specific telemetry should be emitted only from `QueuedPromptsPanelView`, which exists for the `QueueSlashCommand`-enabled regular queue experience:
- `QueuedPrompt.Edited`
- `QueuedPrompt.Deleted`
- `QueuedPrompt.Reordered`
- `QueuedPrompt.PanelCollapseToggled`
If those telemetry events record feature enablement, use `FeatureFlag::QueueSlashCommand`; there is no separate queued-prompt UI feature flag. Regular queueing should keep existing telemetry behavior from slash-command acceptance and prompt submission paths. Do not add new telemetry to the restored legacy placeholder flows.
## Tests
Update tests to cover the single regular queue gate and the legacy placeholder regressions.
Regular queue feature-gate tests:
- With `QueueSlashCommand` disabled, regular queue trigger surfaces remain unavailable and the regular queue panel is not attached.
- With `QueueSlashCommand` enabled, `/queue`, the regular queue workspace action, and the auto-queue flow append to `QueuedQueryModel`.
- With `QueueSlashCommand` enabled and regular queued rows present, `QueuedPromptsPanelView` renders those rows.
- `drain_queued_prompts` runs model drain behavior for regular queued prompts.

Legacy placeholder regression tests:
- Cloud Mode initial / follow-up prompt placeholders use the old pending user query block and never append Cloud Mode rows to `QueuedQueryModel`.
- Cloud Mode lifecycle removal removes the old block when transcript / harness handoff arrives, and keeps it visible across `CloudModeSetupV2` failure tombstones where that was already required.
- `/compact-and` and `/fork-and-compact` restore their legacy pending-user-query placeholder behavior and do not append queue-panel rows.
- Legacy `pending_user_query_view_id` remains confined to pending-user-query placeholder flows, not regular queue rows.

Do not add tests for `NewQueuedPromptUI`; it is not part of the intended architecture.
## Validation
Run:
- `cargo fmt`
- A targeted compile/test pass for the touched client code, preferably the queued prompt and terminal view tests.
- Full presubmit before PR submission.
Do not run the app as part of this change.
## Risks and mitigations
- **Accidentally reintroducing a second queue UI rollout flag**: keep `QueueSlashCommand` as the only regular prompt queue feature gate and do not add `NewQueuedPromptUI`.
- **Migrating compatibility placeholders into the regular queue model**: Cloud Mode placeholders, `/compact-and`, and `/fork-and-compact` stay on restored legacy pending-user-query codepaths.
- **Broadening panel/model scope during restore work**: omit Cloud Mode / compact-only origins, model fields, and non-user-managed panel behavior from this workstream.
- **Mixing regular queue rows with legacy placeholder lifecycle**: `QueuedQueryModel` owns regular Agent Mode prompt queues; pending-user-query rich content owns Cloud Mode and compact follow-up placeholder UI.
