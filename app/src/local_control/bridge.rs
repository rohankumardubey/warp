//! Bridge between protocol-level control requests and Warp application models.
//!
//! The bridge validates protocol version, selectors, credentials, and settings
//! before routing each supported action to an app-side handler.

use ::local_control::auth::CredentialGrant;
use ::local_control::protocol::{
    PaneTarget, TabCloseMode, TabCloseParams, TabTarget, TargetSelector,
};
use ::local_control::{
    Action, ActionKind, ControlError, ErrorCode, InstanceId, RequestEnvelope, ResponseEnvelope,
};
use serde_json::json;
use warpui::platform::TerminationMode;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity, ViewHandle, WindowId};

use crate::local_control::handlers::{app_state, metadata, metadata_config, settings_surfaces};
use crate::local_control::permissions::{
    ensure_action_allowed, ensure_feature_enabled, ensure_protocol_version,
};
use crate::local_control::resolver::{
    target_window_id_for_target, validate_action_params, validate_action_target,
};
use crate::workspace::Workspace;

/// WarpUI model that executes already-authenticated local-control actions.
pub struct LocalControlBridge {
    instance_id: Option<InstanceId>,
}

fn tab_close_mode(action: &Action) -> Result<TabCloseMode, ControlError> {
    Ok(action.params_as::<TabCloseParams>()?.mode)
}

fn validate_empty_params(action: &Action) -> Result<(), ControlError> {
    if action
        .params
        .as_object()
        .is_some_and(serde_json::Map::is_empty)
    {
        return Ok(());
    }
    Err(ControlError::new(
        ErrorCode::InvalidParams,
        format!("{} does not accept parameters", action.kind.as_str()),
    ))
}

fn workspace_for_window(
    window_id: WindowId,
    action: ActionKind,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<ViewHandle<Workspace>, ControlError> {
    ctx.views_of_type::<Workspace>(window_id)
        .and_then(|workspaces| workspaces.into_iter().next())
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::MissingTarget,
                format!(
                    "{} requires a workspace in the target window",
                    action.as_str()
                ),
            )
        })
}

fn tab_index_for_target(
    target: &TargetSelector,
    active_index: usize,
    tab_ids: &[String],
    workspace: &Workspace,
    ctx: &AppContext,
) -> Result<usize, ControlError> {
    match target.tab.as_ref() {
        None | Some(TabTarget::Active) => Ok(active_index),
        Some(TabTarget::Id { id }) => tab_ids
            .iter()
            .position(|tab_id| *tab_id == id.0)
            .ok_or_else(|| {
                ControlError::new(
                    ErrorCode::StaleTarget,
                    "close action cannot resolve the requested tab id",
                )
            }),
        Some(TabTarget::Index { index }) => {
            let index = *index as usize;
            (index < tab_ids.len()).then_some(index).ok_or_else(|| {
                ControlError::new(
                    ErrorCode::StaleTarget,
                    "close action cannot resolve the requested tab index",
                )
            })
        }
        Some(TabTarget::Title { title }) => {
            let matches = workspace
                .tab_views()
                .enumerate()
                .filter_map(|(index, tab)| {
                    (tab.as_ref(ctx).display_title(ctx).as_str() == title).then_some(index)
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [index] => Ok(*index),
                [] => Err(ControlError::new(
                    ErrorCode::MissingTarget,
                    "close action cannot resolve the requested tab title",
                )),
                _ => Err(ControlError::new(
                    ErrorCode::AmbiguousTarget,
                    "close action resolved multiple tabs by title",
                )),
            }
        }
    }
}

fn handle_window_close(
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    validate_empty_params(&request.action)?;
    if request.target.tab.is_some()
        || request.target.pane.is_some()
        || request.target.session.is_some()
    {
        return Err(ControlError::new(
            ErrorCode::InvalidSelector,
            "window.close does not accept tab, pane, or session selectors",
        ));
    }
    let window_id = target_window_id_for_target(ctx, &request.target, ActionKind::WindowClose)?;
    ctx.windows()
        .close_window(window_id, TerminationMode::Cancellable);
    Ok(json!({
        "action": "window.close",
        "ok": true,
    }))
}

fn handle_tab_close(
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    if request.target.pane.is_some() || request.target.session.is_some() {
        return Err(ControlError::new(
            ErrorCode::InvalidSelector,
            "tab.close does not accept pane or session selectors",
        ));
    }
    let mode = tab_close_mode(&request.action)?;
    let window_id = target_window_id_for_target(ctx, &request.target, ActionKind::TabClose)?;
    let workspace = workspace_for_window(window_id, ActionKind::TabClose, ctx)?;
    let closed = workspace.update(ctx, |workspace, ctx| {
        let all_tab_ids = workspace
            .tab_views()
            .map(|tab| tab.id().to_string())
            .collect::<Vec<_>>();
        let selected_index = tab_index_for_target(
            &request.target,
            workspace.active_tab_index(),
            &all_tab_ids,
            workspace,
            ctx,
        )?;
        let tab_indices: Vec<usize> = match mode {
            TabCloseMode::Target => vec![selected_index],
            TabCloseMode::Active => {
                if !matches!(request.target.tab.as_ref(), None | Some(TabTarget::Active)) {
                    return Err(ControlError::new(
                        ErrorCode::InvalidSelector,
                        "tab.close active does not accept a concrete tab selector",
                    ));
                }
                vec![workspace.active_tab_index()]
            }
            TabCloseMode::Others => (0..all_tab_ids.len())
                .filter(|index| *index != selected_index)
                .collect(),
            TabCloseMode::RightOf => ((selected_index + 1)..all_tab_ids.len()).collect(),
        };
        if tab_indices.is_empty() {
            return Ok(true);
        }
        let closed = workspace.close_tabs(
            tab_indices.into_iter(),
            crate::workspace::view::OpenDialogSource::CloseTab {
                tab_index: selected_index,
            },
            false,
            true,
            ctx,
        );
        Ok(closed)
    })?;
    if closed {
        Ok(json!({
            "action": "tab.close",
            "ok": true,
        }))
    } else {
        Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            "tab close was cancelled by an existing app warning",
        ))
    }
}

fn handle_pane_close(
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    validate_empty_params(&request.action)?;
    if request.target.session.is_some() {
        return Err(ControlError::new(
            ErrorCode::InvalidSelector,
            "pane.close does not accept session selectors",
        ));
    }
    let window_id = target_window_id_for_target(ctx, &request.target, ActionKind::PaneClose)?;
    let workspace = workspace_for_window(window_id, ActionKind::PaneClose, ctx)?;
    workspace.update(ctx, |workspace, ctx| {
        let tab_ids = workspace
            .tab_views()
            .map(|tab| tab.id().to_string())
            .collect::<Vec<_>>();
        let tab_index = tab_index_for_target(
            &request.target,
            workspace.active_tab_index(),
            &tab_ids,
            workspace,
            ctx,
        )?;
        let pane_group = workspace
            .get_pane_group_view(tab_index)
            .cloned()
            .ok_or_else(|| {
                ControlError::new(ErrorCode::StaleTarget, "pane.close target tab is stale")
            })?;
        let pane_id = pane_group.read(ctx, |pane_group, ctx| {
            let pane_ids = pane_group.visible_pane_ids();
            match request.target.pane.as_ref() {
                None | Some(PaneTarget::Active) => Ok(pane_group.focused_pane_id(ctx)),
                Some(PaneTarget::Id { id }) => pane_ids
                    .into_iter()
                    .find(|pane_id| pane_id.to_string() == id.0)
                    .ok_or_else(|| {
                        ControlError::new(
                            ErrorCode::StaleTarget,
                            "pane.close cannot resolve the requested pane id",
                        )
                    }),
                Some(PaneTarget::Index { index }) => {
                    pane_ids.into_iter().nth(*index as usize).ok_or_else(|| {
                        ControlError::new(
                            ErrorCode::StaleTarget,
                            "pane.close cannot resolve the requested pane index",
                        )
                    })
                }
            }
        })?;
        pane_group.update(ctx, |pane_group, ctx| pane_group.close_pane(pane_id, ctx));
        Ok::<_, ControlError>(())
    })?;
    Ok(json!({
        "action": "pane.close",
        "ok": true,
    }))
}

impl Entity for LocalControlBridge {
    type Event = ();
}

impl SingletonEntity for LocalControlBridge {}

impl LocalControlBridge {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self { instance_id: None }
    }

    pub(super) fn set_instance_id(&mut self, instance_id: InstanceId) {
        self.instance_id = Some(instance_id);
    }

    pub(super) fn handle_request(
        &mut self,
        request: RequestEnvelope,
        grant: CredentialGrant,
        ctx: &mut ModelContext<Self>,
    ) -> ResponseEnvelope {
        if let Err(error) = ensure_feature_enabled() {
            return ResponseEnvelope::error(request.request_id, error);
        }
        if let Err(error) = ensure_protocol_version(request.protocol_version) {
            return ResponseEnvelope::error(request.request_id, error);
        }
        let Some(instance_id) = &self.instance_id else {
            return ResponseEnvelope::error(
                request.request_id,
                ControlError::new(
                    ErrorCode::BridgeUnavailable,
                    "local-control bridge has no active instance identity",
                ),
            );
        };
        if let Err(error) = validate_request_authority(instance_id, &request.action, &grant) {
            return ResponseEnvelope::error(request.request_id, error);
        }
        if let Err(error) = ensure_action_allowed(request.action.kind, ctx) {
            return ResponseEnvelope::error(request.request_id, error);
        }
        if let Err(error) = validate_action_target(request.action.kind, &request.target) {
            return ResponseEnvelope::error(request.request_id, error);
        }
        let result = match request.action.kind {
            ActionKind::InstanceList => metadata::instance(&self.instance_id),
            ActionKind::InstanceInspect => metadata::inspect(&self.instance_id, ctx),
            ActionKind::AppPing => metadata::ping(&self.instance_id),
            ActionKind::AppVersion => metadata::version(&self.instance_id),
            ActionKind::AppActive => metadata::active(&self.instance_id, ctx),
            ActionKind::CapabilityList => Ok(metadata::capability_list()),
            ActionKind::CapabilityInspect => metadata::capability_inspect(&request.action),
            ActionKind::ActionList => Ok(metadata::action_list()),
            ActionKind::ActionInspect => metadata::action_inspect(&request.action),
            ActionKind::WindowList => metadata::window_list(&request.target, ctx),
            ActionKind::WindowInspect => metadata::window_inspect(&request.target, ctx),
            ActionKind::TabList => metadata::tab_list(&request.target, ctx),
            ActionKind::TabInspect => metadata::tab_inspect(&request.target, ctx),
            ActionKind::AppFocus
            | ActionKind::WindowCreate
            | ActionKind::WindowFocus
            | ActionKind::TabCreate
            | ActionKind::TabActivate
            | ActionKind::TabMove
            | ActionKind::PaneSplit
            | ActionKind::PaneFocus
            | ActionKind::PaneNavigate
            | ActionKind::PaneResize
            | ActionKind::PaneMaximize
            | ActionKind::PaneUnmaximize
            | ActionKind::SessionActivate
            | ActionKind::SessionPrevious
            | ActionKind::SessionNext
            | ActionKind::SessionReopenClosed
            | ActionKind::InputInsert
            | ActionKind::InputReplace
            | ActionKind::SurfaceSettingsOpen
            | ActionKind::SurfaceCommandPaletteOpen
            | ActionKind::SurfaceCommandSearchOpen
            | ActionKind::SurfaceWarpDriveOpen
            | ActionKind::SurfaceWarpDriveToggle
            | ActionKind::SurfaceResourceCenterToggle
            | ActionKind::SurfaceAiAssistantToggle
            | ActionKind::SurfaceCodeReviewToggle
            | ActionKind::SurfaceLeftPanelToggle
            | ActionKind::SurfaceRightPanelToggle
            | ActionKind::SurfaceVerticalTabsToggle
            | ActionKind::FileOpen => app_state::handle(
                &self.instance_id,
                request.action.kind,
                &request.action.params,
                &request.target,
                ctx,
            ),
            ActionKind::TabRename => metadata_config::tab_rename(
                &self.instance_id,
                &request.target,
                &request.action,
                ctx,
            ),
            ActionKind::TabResetName => {
                metadata_config::tab_reset_name(&self.instance_id, &request.target, ctx)
            }
            ActionKind::TabColorSet => metadata_config::tab_color_set(
                &self.instance_id,
                &request.target,
                &request.action,
                ctx,
            ),
            ActionKind::TabColorClear => {
                metadata_config::tab_color_clear(&self.instance_id, &request.target, ctx)
            }
            ActionKind::PaneList => metadata::pane_list(&request.target, ctx),
            ActionKind::PaneInspect => metadata::pane_inspect(&request.target, ctx),
            ActionKind::PaneRename => metadata_config::pane_rename(
                &self.instance_id,
                &request.target,
                &request.action,
                ctx,
            ),
            ActionKind::PaneResetName => {
                metadata_config::pane_reset_name(&self.instance_id, &request.target, ctx)
            }
            ActionKind::SessionList => metadata::session_list(&request.target, ctx),
            ActionKind::SessionInspect => metadata::session_inspect(&request.target, ctx),
            ActionKind::ThemeList => settings_surfaces::theme_list(ctx),
            ActionKind::ThemeGet => settings_surfaces::theme_get(ctx),
            ActionKind::ThemeSet
            | ActionKind::ThemeSystemSet
            | ActionKind::ThemeLightSet
            | ActionKind::ThemeDarkSet => {
                metadata_config::theme_set(request.action.kind, &request.action, ctx)
            }
            ActionKind::AppearanceGet => settings_surfaces::appearance_get(ctx),
            ActionKind::AppearanceFontSizeIncrease
            | ActionKind::AppearanceFontSizeDecrease
            | ActionKind::AppearanceFontSizeReset
            | ActionKind::AppearanceZoomIncrease
            | ActionKind::AppearanceZoomDecrease
            | ActionKind::AppearanceZoomReset => {
                metadata_config::appearance_mutation(request.action.kind, ctx)
            }
            ActionKind::SettingList => settings_surfaces::setting_list(&request.action, ctx),
            ActionKind::SettingGet => settings_surfaces::setting_get(&request.action, ctx),
            ActionKind::SettingSet => metadata_config::setting_set(&request.action, ctx),
            ActionKind::SettingToggle => metadata_config::setting_toggle(&request.action, ctx),
            ActionKind::KeybindingList => settings_surfaces::keybinding_list(ctx),
            ActionKind::KeybindingGet => settings_surfaces::keybinding_get(&request.action, ctx),
            ActionKind::WindowClose => handle_window_close(&request, ctx),
            ActionKind::TabClose => handle_tab_close(&request, ctx),
            ActionKind::PaneClose => handle_pane_close(&request, ctx),
        };
        match result {
            Ok(data) => ResponseEnvelope::ok(request.request_id, data),
            Err(error) => ResponseEnvelope::error(request.request_id, error),
        }
    }
}

pub(crate) fn validate_request_authority(
    instance_id: &InstanceId,
    action: &Action,
    grant: &CredentialGrant,
) -> Result<(), ControlError> {
    grant.verify_for_action(instance_id, action.kind)?;
    if !action.kind.is_implemented() {
        return Err(ControlError::new(
            ErrorCode::UnsupportedAction,
            format!(
                "{} is not implemented by this local-control bridge",
                action.kind.as_str()
            ),
        ));
    }
    validate_action_params(action)
}
