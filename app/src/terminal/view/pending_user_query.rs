use warp_core::features::FeatureFlag;
use warpui::{SingletonEntity, ViewContext};

use super::rich_content::RichContentMetadata;
use crate::{
    ai::agent::conversation::AIConversationId,
    ai::blocklist::block::FinishReason,
    ai::blocklist::block::{PendingUserQueryBlock, PendingUserQueryBlockEvent},
    auth::AuthStateProvider,
    terminal::{view::PendingUserQueryKind, TerminalView},
};

impl TerminalView {
    pub(in crate::terminal::view) fn pending_user_query_conversation_id(
        &self,
    ) -> Option<AIConversationId> {
        self.pending_user_query_conversation_id
    }
    fn insert_pending_user_query_block(
        &mut self,
        conversation_id: Option<AIConversationId>,
        prompt: String,
        show_close_button: bool,
        show_send_now_button: bool,
        kind: PendingUserQueryKind,
        ctx: &mut ViewContext<Self>,
    ) {
        self.remove_pending_user_query_block(ctx);
        self.pending_user_query_kind = Some(kind);
        self.pending_user_query_conversation_id = conversation_id;
        let auth_state = AuthStateProvider::as_ref(ctx).get().clone();
        let user_display_name = auth_state
            .username_for_display()
            .unwrap_or_else(|| "User".to_owned());
        let profile_image_path = auth_state.user_photo_url();
        let prompt_for_callback = prompt.clone();

        let handle = ctx.add_typed_action_view(|ctx| {
            PendingUserQueryBlock::new(
                prompt,
                user_display_name,
                profile_image_path,
                show_close_button,
                show_send_now_button,
                ctx,
            )
        });
        ctx.subscribe_to_view(&handle, move |me, block, event, ctx| match event {
            PendingUserQueryBlockEvent::Dismissed => {
                if show_close_button {
                    me.remove_pending_user_query_block(ctx);
                }
            }
            PendingUserQueryBlockEvent::SendNow => {
                if show_send_now_button {
                    if let Some(conversation_id) = conversation_id {
                        me.send_queued_prompt_now(
                            conversation_id,
                            prompt_for_callback.clone(),
                            ctx,
                        );
                    }
                }
            }
            PendingUserQueryBlockEvent::TextSelected => {
                // Ensure only one active text selection across the entire terminal view.
                me.clear_selected_text_except(Some(block.id()), ctx);
            }
        });
        let view_id = handle.id();

        self.insert_rich_content(
            None,
            handle.clone(),
            Some(RichContentMetadata::PendingUserQuery {
                pending_user_query_block_handle: handle,
            }),
            super::rich_content::RichContentInsertionPosition::PinToBottom,
            ctx,
        );
        self.pending_user_query_view_id = Some(view_id);
    }

    /// Inserts a pending user query block for a Cloud Mode run whose real user query has not yet
    /// arrived in the shared-session transcript.
    /// The block shows the user's prompt with a "Queued" badge and no buttons: the
    /// queued state is owned by the run's lifecycle, not by a local `/queue`-style callback, so
    /// the prompt is not re-submitted when the block is removed.
    pub(in crate::terminal::view) fn insert_cloud_mode_queued_user_query_block(
        &mut self,
        prompt: String,
        ctx: &mut ViewContext<Self>,
    ) {
        let conversation_id = self
            .ai_context_model
            .as_ref(ctx)
            .selected_conversation_id(ctx);
        self.insert_pending_user_query_block(
            conversation_id,
            prompt,
            /* show_close_button */ false,
            /* show_send_now_button */ false,
            PendingUserQueryKind::CloudMode,
            ctx,
        );
    }

    /// Removes the pending user query block, if one exists. No-op if none is present.
    pub(super) fn remove_pending_user_query_block(&mut self, ctx: &mut ViewContext<Self>) {
        self.pending_user_query_kind = None;
        self.pending_user_query_conversation_id = None;
        self.queued_prompt_callback = None;
        if let Some(view_id) = self.pending_user_query_view_id.take() {
            self.model
                .lock()
                .block_list_mut()
                .remove_rich_content(view_id);
            self.rich_content_views.retain(|rc| rc.view_id() != view_id);
            ctx.notify();
        }
    }

    fn send_queued_prompt_now(
        &mut self,
        conversation_id: AIConversationId,
        prompt: String,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.queued_prompt_callback.take().is_some() {
            self.remove_pending_user_query_block(ctx);
            self.ai_controller.update(ctx, |controller, ctx| {
                controller.send_user_query_in_conversation_no_lrc_subagent(
                    prompt,
                    conversation_id,
                    None,
                    ctx,
                );
            });
        }
    }

    pub fn send_user_query_after_next_conversation_finished(
        &mut self,
        prompt: String,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(conversation_id) = self
            .ai_context_model
            .as_ref(ctx)
            .selected_conversation_id(ctx)
        else {
            return;
        };

        let prompt_for_callback = prompt.clone();
        self.queued_prompt_callback = Some(Box::new(move |view, reason, ctx| {
            view.remove_pending_user_query_block(ctx);
            match reason {
                FinishReason::Complete => {
                    view.ai_controller.update(ctx, |controller, ctx| {
                        controller.send_user_query_in_conversation_no_lrc_subagent(
                            prompt_for_callback,
                            conversation_id,
                            None,
                            ctx,
                        );
                    });
                }
                FinishReason::Cancelled
                | FinishReason::CancelledDuringRequestedCommandExecution
                | FinishReason::Error => {
                    view.input.update(ctx, |input, ctx| {
                        input.set_input_mode_agent(true, ctx);
                        input.replace_buffer_content(&prompt_for_callback, ctx);
                    });
                }
            }
        }));

        if FeatureFlag::PendingUserQueryIndicator.is_enabled() {
            self.insert_pending_user_query_block(
                Some(conversation_id),
                prompt,
                /* show_close_button */ true,
                /* show_send_now_button */ true,
                PendingUserQueryKind::QueuedPrompt,
                ctx,
            );
        }
    }
}
