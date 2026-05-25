use ::local_control::protocol::{
    DriveCreateParams, DriveDeleteParams, DriveInsertParams, DriveMutationAudit,
    DriveMutationResult, DriveObjectSummary, DriveObjectType as ControlDriveObjectType,
    DriveShareToTeamParams, DriveTarget, DriveUpdateParams, PermissionCategory, TargetSelector,
};
use ::local_control::{ActionKind, ControlError, ErrorCode, RequestEnvelope};
use warpui::{ModelContext, SingletonEntity};

use crate::auth::AuthStateProvider;
use crate::cloud_object::model::generic_string_model::GenericStringObjectId;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::{
    CloudObject, GenericStringObjectFormat, JsonObjectType, ObjectType, Owner, Space,
};
use crate::drive::folders::{CloudFolder, CloudFolderModel, FolderId};
use crate::drive::CloudObjectTypeAndId;
use crate::env_vars::{CloudEnvVarCollection, CloudEnvVarCollectionModel, EnvVarCollection};
use crate::local_control::LocalControlBridge;
use crate::notebooks::{CloudNotebook, CloudNotebookModel, NotebookId};
use crate::server::cloud_objects::update_manager::UpdateManager;
use crate::server::ids::{ClientId, SyncId};
use crate::workflows::workflow::Workflow;
use crate::workflows::{CloudWorkflow, CloudWorkflowModel, WorkflowId};
use crate::workspaces::user_workspaces::UserWorkspaces;

pub(crate) fn create_drive_object(
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    validate_drive_target(&request.target, request.action.kind)?;
    let params = request.action.params_as::<DriveCreateParams>()?;
    if params.name.is_empty() {
        return Err(ControlError::new(
            ErrorCode::InvalidParams,
            "drive.object.create requires a non-empty Drive object name",
        ));
    }
    let subject = authenticated_user_subject(ctx)?;
    let owner = authenticated_user_owner(ctx)?;
    let client_id = ClientId::new();
    let sync_id = SyncId::ClientId(client_id);
    CloudModel::handle(ctx).update(ctx, |cloud_model, ctx| match params.object_type {
        ControlDriveObjectType::Workflow | ControlDriveObjectType::Prompt => {
            let workflow =
                workflow_from_drive_content(params.object_type, &params.name, params.content)?;
            cloud_model.create_object(
                sync_id,
                CloudWorkflow::new_local(CloudWorkflowModel::new(workflow), owner, None, client_id),
                ctx,
            );
            Ok(())
        }
        ControlDriveObjectType::Notebook => {
            let notebook = notebook_from_drive_content(&params.name, params.content, None)?;
            cloud_model.create_object(
                sync_id,
                CloudNotebook::new_local(notebook, owner, None, client_id),
                ctx,
            );
            Ok(())
        }
        ControlDriveObjectType::EnvVarCollection => {
            let env_vars = env_vars_from_drive_content(&params.name, params.content)?;
            cloud_model.create_object(
                sync_id,
                CloudEnvVarCollection::new_local(
                    CloudEnvVarCollectionModel::new(env_vars),
                    owner,
                    None,
                    client_id,
                ),
                ctx,
            );
            Ok(())
        }
        ControlDriveObjectType::Folder => {
            cloud_model.create_object(
                sync_id,
                CloudFolder::new_local(
                    CloudFolderModel::new(&params.name, false),
                    owner,
                    None,
                    client_id,
                ),
                ctx,
            );
            Ok(())
        }
    })?;
    let cloud_model = CloudModel::as_ref(ctx);
    let object = cloud_model.get_by_uid(&sync_id.uid()).ok_or_else(|| {
        ControlError::new(
            ErrorCode::Internal,
            "drive.object.create could not resolve the created Drive object",
        )
    })?;
    drive_mutation_result(object, params.object_type, request.action.kind, subject)
}

pub(crate) fn update_drive_object(
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    validate_drive_target(&request.target, request.action.kind)?;
    let params = request.action.params_as::<DriveUpdateParams>()?;
    validate_drive_request_id(&params.id, request.action.kind)?;
    validate_drive_target_matches_params(
        &request.target,
        params.object_type,
        &params.id,
        request.action.kind,
    )?;
    let subject = authenticated_user_subject(ctx)?;
    let (sync_id, existing_notebook) = {
        let cloud_model = CloudModel::as_ref(ctx);
        let object = drive_object_for_mutation(
            cloud_model,
            params.object_type,
            &params.id,
            request.action.kind,
        )?;
        (
            object.sync_id(),
            object
                .as_any()
                .downcast_ref::<CloudNotebook>()
                .map(|notebook| notebook.model().clone()),
        )
    };
    CloudModel::handle(ctx).update(ctx, |cloud_model, ctx| match params.object_type {
        ControlDriveObjectType::Workflow | ControlDriveObjectType::Prompt => {
            let workflow =
                workflow_from_drive_content(params.object_type, "", params.content.clone())?;
            cloud_model.update_object_from_edit::<WorkflowId, CloudWorkflowModel>(
                CloudWorkflowModel::new(workflow),
                sync_id,
                ctx,
            );
            Ok(())
        }
        ControlDriveObjectType::Notebook => {
            let notebook =
                notebook_from_drive_content("", params.content.clone(), existing_notebook)?;
            cloud_model
                .update_object_from_edit::<NotebookId, CloudNotebookModel>(notebook, sync_id, ctx);
            Ok(())
        }
        ControlDriveObjectType::EnvVarCollection => {
            let env_vars = env_vars_from_drive_content("", params.content.clone())?;
            cloud_model
                .update_object_from_edit::<GenericStringObjectId, CloudEnvVarCollectionModel>(
                    CloudEnvVarCollectionModel::new(env_vars),
                    sync_id,
                    ctx,
                );
            Ok(())
        }
        ControlDriveObjectType::Folder => {
            let name = params
                .content
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    ControlError::new(
                        ErrorCode::InvalidParams,
                        "drive.object.update folder content requires a name string",
                    )
                })?;
            cloud_model.update_object_from_edit::<FolderId, CloudFolderModel>(
                CloudFolderModel::new(name, false),
                sync_id,
                ctx,
            );
            Ok(())
        }
    })?;
    let cloud_model = CloudModel::as_ref(ctx);
    let object = drive_object_for_mutation(
        cloud_model,
        params.object_type,
        &params.id,
        request.action.kind,
    )?;
    drive_mutation_result(object, params.object_type, request.action.kind, subject)
}

pub(crate) fn delete_drive_object(
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    validate_drive_target(&request.target, request.action.kind)?;
    let params = request.action.params_as::<DriveDeleteParams>()?;
    validate_drive_request_id(&params.id, request.action.kind)?;
    validate_drive_target_matches_params(
        &request.target,
        params.object_type,
        &params.id,
        request.action.kind,
    )?;
    let subject = authenticated_user_subject(ctx)?;
    let (sync_id, summary) = {
        let cloud_model = CloudModel::as_ref(ctx);
        let object = drive_object_for_mutation(
            cloud_model,
            params.object_type,
            &params.id,
            request.action.kind,
        )?;
        let summary = drive_object_summary(object).ok_or_else(|| {
            ControlError::new(
                ErrorCode::UnsupportedAction,
                "drive.object.delete does not support this Drive object type",
            )
        })?;
        (object.sync_id(), summary)
    };
    CloudModel::handle(ctx).update(ctx, |cloud_model, ctx| {
        cloud_model.delete_object(sync_id, ctx);
    });
    to_drive_data(DriveMutationResult {
        object: summary,
        audit: Some(audit(request.action.kind, subject)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::local_control::protocol::{DriveObjectSelector, WindowTarget};

    fn target_for(object_type: ControlDriveObjectType, id: &str) -> TargetSelector {
        TargetSelector {
            drive: Some(DriveTarget::Id {
                object_type,
                id: DriveObjectSelector(id.to_owned()),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn drive_mutations_require_deterministic_id_selector() {
        let err = validate_drive_target(&TargetSelector::default(), ActionKind::DriveObjectUpdate)
            .expect_err("missing Drive id selector is rejected");
        assert_eq!(err.code, ErrorCode::MissingTarget);

        let err = validate_drive_target(
            &TargetSelector {
                drive: Some(DriveTarget::Name {
                    object_type: ControlDriveObjectType::Notebook,
                    name: "notes".to_owned(),
                }),
                ..Default::default()
            },
            ActionKind::DriveObjectUpdate,
        )
        .expect_err("Drive name selectors are rejected for mutations");
        assert_eq!(err.code, ErrorCode::UnsupportedAction);

        validate_drive_target(
            &target_for(ControlDriveObjectType::Notebook, "notebook_1"),
            ActionKind::DriveObjectUpdate,
        )
        .expect("id selector is accepted");
    }

    #[test]
    fn drive_mutations_reject_non_drive_selectors() {
        let err = validate_drive_target(
            &TargetSelector {
                window: Some(WindowTarget::Active),
                drive: Some(DriveTarget::Id {
                    object_type: ControlDriveObjectType::Notebook,
                    id: DriveObjectSelector("notebook_1".to_owned()),
                }),
                ..Default::default()
            },
            ActionKind::DriveObjectUpdate,
        )
        .expect_err("window selector is rejected");
        assert_eq!(err.code, ErrorCode::InvalidSelector);
    }

    #[test]
    fn drive_target_must_match_params() {
        validate_drive_target_matches_params(
            &target_for(ControlDriveObjectType::Notebook, "notebook_1"),
            ControlDriveObjectType::Notebook,
            "notebook_1",
            ActionKind::DriveObjectUpdate,
        )
        .expect("matching selector and params are accepted");

        let err = validate_drive_target_matches_params(
            &target_for(ControlDriveObjectType::Workflow, "workflow_1"),
            ControlDriveObjectType::Notebook,
            "notebook_1",
            ActionKind::DriveObjectUpdate,
        )
        .expect_err("mismatched selector and params are rejected");
        assert_eq!(err.code, ErrorCode::TargetStateConflict);
    }
}

pub(crate) fn insert_drive_object(
    request: &RequestEnvelope,
) -> Result<serde_json::Value, ControlError> {
    validate_drive_target(&request.target, request.action.kind)?;
    let params = request.action.params_as::<DriveInsertParams>()?;
    validate_drive_request_id(&params.id, request.action.kind)?;
    validate_drive_target_matches_params(
        &request.target,
        params.object_type,
        &params.id,
        request.action.kind,
    )?;
    if params.object_type != ControlDriveObjectType::Notebook {
        return Err(ControlError::new(
            ErrorCode::UnsupportedAction,
            "drive.object.insert only supports notebook objects in this shard",
        ));
    }
    Err(ControlError::new(
        ErrorCode::ExecutionContextNotAllowed,
        "drive.object.insert requires an insertion target policy hook that is not available in this shard",
    ))
}

pub(crate) fn share_drive_object_to_team(
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    validate_drive_target(&request.target, request.action.kind)?;
    let params = request.action.params_as::<DriveShareToTeamParams>()?;
    validate_drive_request_id(&params.id, request.action.kind)?;
    validate_drive_target_matches_params(
        &request.target,
        params.object_type,
        &params.id,
        request.action.kind,
    )?;
    let subject = authenticated_user_subject(ctx)?;
    let team_uid = UserWorkspaces::as_ref(ctx)
        .current_team_uid()
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::MissingTarget,
                "drive.object.share-to-team requires a current Warp team",
            )
        })?;
    let (type_and_id, summary) = {
        let cloud_model = CloudModel::as_ref(ctx);
        let object = drive_object_for_mutation(
            cloud_model,
            params.object_type,
            &params.id,
            request.action.kind,
        )?;
        if !matches!(object.permissions().owner, Owner::User { .. }) {
            return Err(ControlError::new(
                ErrorCode::TargetStateConflict,
                "drive.object.share-to-team only supports personal Drive objects",
            ));
        }
        let Some(server_id) = object.sync_id().into_server() else {
            return Err(ControlError::new(
                ErrorCode::TargetStateConflict,
                "drive.object.share-to-team requires a server-backed Drive object",
            ));
        };
        let summary = drive_object_summary(object).ok_or_else(|| {
            ControlError::new(
                ErrorCode::UnsupportedAction,
                "drive.object.share-to-team does not support this Drive object type",
            )
        })?;
        (
            CloudObjectTypeAndId::from_id_and_type(
                SyncId::ServerId(server_id),
                object.object_type(),
            ),
            summary,
        )
    };
    UpdateManager::handle(ctx).update(ctx, |update_manager, ctx| {
        update_manager.move_object_to_location(
            type_and_id,
            crate::cloud_object::CloudObjectLocation::Space(Space::Team { team_uid }),
            ctx,
        );
    });
    to_drive_data(DriveMutationResult {
        object: summary,
        audit: Some(audit(request.action.kind, subject)),
    })
}

fn authenticated_user_subject(
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<String, ControlError> {
    let auth_state = AuthStateProvider::as_ref(ctx).get();
    if auth_state.is_anonymous_or_logged_out() {
        return Err(ControlError::new(
            ErrorCode::AuthenticatedUserUnavailable,
            "this action requires a logged-in Warp user",
        ));
    }
    auth_state
        .user_id()
        .map(|uid| uid.as_string())
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::AuthenticatedUserUnavailable,
                "this action requires a logged-in Warp user",
            )
        })
}

fn authenticated_user_owner(
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<Owner, ControlError> {
    let auth_state = AuthStateProvider::as_ref(ctx).get();
    auth_state
        .user_id()
        .map(|user_uid| Owner::User { user_uid })
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::AuthenticatedUserUnavailable,
                "this action requires a logged-in Warp user",
            )
        })
}

#[derive(serde::Deserialize)]
struct NotebookDriveContent {
    title: Option<String>,
    data: Option<String>,
}

fn workflow_from_drive_content(
    object_type: ControlDriveObjectType,
    fallback_name: &str,
    content: serde_json::Value,
) -> Result<Workflow, ControlError> {
    if let Ok(mut workflow) = serde_json::from_value::<Workflow>(content.clone()) {
        if workflow_kind_matches(object_type, &workflow) {
            if !fallback_name.is_empty() {
                workflow.set_name(fallback_name);
            }
            return Ok(workflow);
        }
        return Err(ControlError::new(
            ErrorCode::InvalidParams,
            "Drive workflow content does not match the requested object type",
        ));
    }
    match object_type {
        ControlDriveObjectType::Workflow => {
            let command = content.get("command").and_then(serde_json::Value::as_str);
            let command = command.ok_or_else(|| {
                ControlError::new(
                    ErrorCode::InvalidParams,
                    "drive.object.create/update workflow content requires a command string or typed workflow object",
                )
            })?;
            Ok(Workflow::new(fallback_name, command))
        }
        ControlDriveObjectType::Prompt => {
            let query = content.get("query").and_then(serde_json::Value::as_str);
            let query = query.ok_or_else(|| {
                ControlError::new(
                    ErrorCode::InvalidParams,
                    "drive.object.create/update prompt content requires a query string or typed workflow object",
                )
            })?;
            Ok(Workflow::AgentMode {
                name: fallback_name.to_owned(),
                query: query.to_owned(),
                description: None,
                arguments: Vec::new(),
            })
        }
        _ => Err(ControlError::new(
            ErrorCode::UnsupportedAction,
            "workflow content is only valid for workflow and prompt Drive object types",
        )),
    }
}

fn workflow_kind_matches(object_type: ControlDriveObjectType, workflow: &Workflow) -> bool {
    match object_type {
        ControlDriveObjectType::Workflow => workflow.is_command_workflow(),
        ControlDriveObjectType::Prompt => workflow.is_agent_mode_workflow(),
        _ => false,
    }
}

fn notebook_from_drive_content(
    fallback_title: &str,
    content: serde_json::Value,
    existing: Option<CloudNotebookModel>,
) -> Result<CloudNotebookModel, ControlError> {
    if let Some(data) = content.as_str() {
        return Ok(CloudNotebookModel {
            title: non_empty_string(fallback_title)
                .or_else(|| existing.as_ref().map(|notebook| notebook.title.clone()))
                .unwrap_or_default(),
            data: data.to_owned(),
            ai_document_id: existing
                .as_ref()
                .and_then(|notebook| notebook.ai_document_id),
            conversation_id: existing.and_then(|notebook| notebook.conversation_id),
        });
    }
    let typed = serde_json::from_value::<NotebookDriveContent>(content).map_err(|err| {
        ControlError::with_details(
            ErrorCode::InvalidParams,
            "drive.object.create/update notebook content requires a string or typed notebook object",
            err.to_string(),
        )
    })?;
    Ok(CloudNotebookModel {
        title: typed
            .title
            .or_else(|| non_empty_string(fallback_title))
            .or_else(|| existing.as_ref().map(|notebook| notebook.title.clone()))
            .unwrap_or_default(),
        data: typed
            .data
            .or_else(|| existing.as_ref().map(|notebook| notebook.data.clone()))
            .unwrap_or_default(),
        ai_document_id: existing
            .as_ref()
            .and_then(|notebook| notebook.ai_document_id),
        conversation_id: existing.and_then(|notebook| notebook.conversation_id),
    })
}

fn env_vars_from_drive_content(
    fallback_title: &str,
    content: serde_json::Value,
) -> Result<EnvVarCollection, ControlError> {
    let mut env_vars = serde_json::from_value::<EnvVarCollection>(content).map_err(|err| {
        ControlError::with_details(
            ErrorCode::InvalidParams,
            "drive.object.create/update env-var-collection content requires a typed environment-variable collection",
            err.to_string(),
        )
    })?;
    if env_vars.title.as_ref().is_none_or(String::is_empty) {
        env_vars.title = non_empty_string(fallback_title);
    }
    Ok(env_vars)
}

fn non_empty_string(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn validate_drive_request_id(id: &str, action: ActionKind) -> Result<(), ControlError> {
    if id.is_empty() {
        return Err(ControlError::new(
            ErrorCode::InvalidParams,
            format!("{} requires a non-empty Drive object id", action.as_str()),
        ));
    }
    Ok(())
}

fn validate_drive_target_matches_params(
    target: &TargetSelector,
    object_type: ControlDriveObjectType,
    id: &str,
    action: ActionKind,
) -> Result<(), ControlError> {
    if let Some(DriveTarget::Id {
        object_type: target_type,
        id: target_id,
    }) = target.drive.as_ref()
    {
        if *target_type != object_type || target_id.0 != id {
            return Err(ControlError::new(
                ErrorCode::TargetStateConflict,
                format!(
                    "{} target selector does not match the requested Drive object",
                    action.as_str()
                ),
            ));
        }
    }
    Ok(())
}

fn drive_object_for_mutation<'a>(
    cloud_model: &'a CloudModel,
    object_type: ControlDriveObjectType,
    id: &str,
    action: ActionKind,
) -> Result<&'a dyn CloudObject, ControlError> {
    let object = cloud_model.get_by_uid(&id.to_owned()).ok_or_else(|| {
        ControlError::new(
            ErrorCode::StaleTarget,
            format!(
                "{} could not resolve the requested Drive object id",
                action.as_str()
            ),
        )
    })?;
    let summary = drive_object_summary(object).ok_or_else(|| {
        ControlError::new(
            ErrorCode::UnsupportedAction,
            format!(
                "{} does not support this Drive object type",
                action.as_str()
            ),
        )
    })?;
    if summary.object_type != object_type {
        return Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            format!(
                "{} Drive object type does not match the requested type",
                action.as_str()
            ),
        ));
    }
    Ok(object)
}

fn drive_mutation_result(
    object: &dyn CloudObject,
    object_type: ControlDriveObjectType,
    action: ActionKind,
    subject: String,
) -> Result<serde_json::Value, ControlError> {
    let summary = drive_object_summary(object).ok_or_else(|| {
        ControlError::new(
            ErrorCode::UnsupportedAction,
            "Drive mutation does not support this Drive object type",
        )
    })?;
    if summary.object_type != object_type {
        return Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            "Drive object type does not match the requested type",
        ));
    }
    to_drive_data(DriveMutationResult {
        object: summary,
        audit: Some(audit(action, subject)),
    })
}

fn drive_object_summary(object: &dyn CloudObject) -> Option<DriveObjectSummary> {
    Some(DriveObjectSummary {
        object_type: control_drive_object_type(object)?,
        id: object.uid(),
        name: object.display_name(),
    })
}

fn validate_drive_target(target: &TargetSelector, action: ActionKind) -> Result<(), ControlError> {
    if target.window.is_some() || target.tab.is_some() || target.pane.is_some() {
        return Err(ControlError::new(
            ErrorCode::InvalidSelector,
            format!(
                "{} does not accept window, tab, or pane selectors",
                action.as_str()
            ),
        ));
    }
    match (action, target.drive.as_ref()) {
        (ActionKind::DriveObjectCreate, None) => Ok(()),
        (ActionKind::DriveObjectCreate, Some(_)) => Err(ControlError::new(
            ErrorCode::UnsupportedAction,
            "drive.object.create uses explicit parameters and does not accept Drive selectors",
        )),
        (
            ActionKind::DriveObjectUpdate
            | ActionKind::DriveObjectDelete
            | ActionKind::DriveObjectInsert
            | ActionKind::DriveObjectShareToTeam,
            Some(DriveTarget::Id { id, .. }),
        ) => {
            if id.0.is_empty() {
                return Err(ControlError::new(
                    ErrorCode::InvalidSelector,
                    format!(
                        "{} requires a non-empty Drive object id selector",
                        action.as_str()
                    ),
                ));
            }
            Ok(())
        }
        (
            ActionKind::DriveObjectUpdate
            | ActionKind::DriveObjectDelete
            | ActionKind::DriveObjectInsert
            | ActionKind::DriveObjectShareToTeam,
            Some(DriveTarget::Name { .. }),
        ) => Err(ControlError::new(
            ErrorCode::UnsupportedAction,
            format!("{} does not support Drive name selectors", action.as_str()),
        )),
        (
            ActionKind::DriveObjectUpdate
            | ActionKind::DriveObjectDelete
            | ActionKind::DriveObjectInsert
            | ActionKind::DriveObjectShareToTeam,
            None,
        ) => Err(ControlError::new(
            ErrorCode::MissingTarget,
            format!(
                "{} requires a deterministic Drive id selector",
                action.as_str()
            ),
        )),
        (_, _) => Ok(()),
    }
}

fn control_drive_object_type(object: &dyn CloudObject) -> Option<ControlDriveObjectType> {
    match object.object_type() {
        ObjectType::Workflow => {
            let workflow = object.as_any().downcast_ref::<CloudWorkflow>()?;
            if workflow.model().data.is_agent_mode_workflow() {
                Some(ControlDriveObjectType::Prompt)
            } else {
                Some(ControlDriveObjectType::Workflow)
            }
        }
        ObjectType::Notebook => Some(ControlDriveObjectType::Notebook),
        ObjectType::Folder => Some(ControlDriveObjectType::Folder),
        ObjectType::GenericStringObject(GenericStringObjectFormat::Json(
            JsonObjectType::EnvVarCollection,
        )) => Some(ControlDriveObjectType::EnvVarCollection),
        _ => None,
    }
}

fn audit(action: ActionKind, authenticated_user_subject: String) -> DriveMutationAudit {
    DriveMutationAudit {
        action: action.as_str().to_owned(),
        authenticated_user_subject,
        permission_category: PermissionCategory::MutateUnderlyingData,
    }
}

fn to_drive_data<T: serde::Serialize>(data: T) -> Result<serde_json::Value, ControlError> {
    serde_json::to_value(data).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to encode local-control Drive response",
            err.to_string(),
        )
    })
}
