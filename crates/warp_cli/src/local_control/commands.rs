//! Implementations for user-facing `warpctrl` command groups.
use local_control::protocol::{
    Action, ActionKind, ActionMetadata, ControlError, DriveCreateParams, DriveDeleteParams,
    DriveInsertParams, DriveObjectSelector, DriveObjectType, DriveShareToTeamParams, DriveTarget,
    DriveUpdateParams, ErrorCode, RequestEnvelope, TargetSelector,
};
use local_control::selection::select_instance;
use serde::Serialize;
use serde_json::json;

use crate::agent::OutputFormat;
use crate::local_control::output::{write_json, write_json_line};
use crate::local_control::selectors::instance_selector;
use crate::local_control::{
    AppCommand, DriveCliObjectType, DriveCommand, DriveObjectCommand, DriveObjectContentArgs,
    DriveObjectIdArgs, InstanceCommand, TabCommand, TargetArgs,
};

/// Display-oriented projection of a discoverable Warp instance.
#[derive(Serialize)]
struct InstanceSummary {
    instance_id: String,
    pid: u32,
    channel: String,
    app_id: String,
    app_version: Option<String>,
    started_at: String,
    endpoint: Option<local_control::discovery::ControlEndpoint>,
    outside_warp_control_enabled: bool,
    actions: Vec<ActionMetadata>,
}

impl From<local_control::discovery::InstanceRecord> for InstanceSummary {
    fn from(record: local_control::discovery::InstanceRecord) -> Self {
        Self {
            instance_id: record.instance_id.0,
            pid: record.pid,
            channel: record.channel,
            app_id: record.app_id,
            app_version: record.app_version,
            started_at: record.started_at.to_rfc3339(),
            endpoint: record.endpoint,
            outside_warp_control_enabled: record.outside_warp_control_enabled,
            actions: record.actions,
        }
    }
}

impl From<DriveCliObjectType> for DriveObjectType {
    fn from(value: DriveCliObjectType) -> Self {
        match value {
            DriveCliObjectType::Workflow => Self::Workflow,
            DriveCliObjectType::Notebook => Self::Notebook,
            DriveCliObjectType::EnvVarCollection => Self::EnvVarCollection,
            DriveCliObjectType::Prompt => Self::Prompt,
            DriveCliObjectType::Folder => Self::Folder,
        }
    }
}

pub(super) fn run_instance_command(
    command: InstanceCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        InstanceCommand::List => {
            let summaries = local_control::discovery::list_instances()
                .into_iter()
                .map(InstanceSummary::from)
                .collect::<Vec<_>>();
            match output_format {
                OutputFormat::Json => write_json(&summaries),
                OutputFormat::Ndjson => {
                    for summary in summaries {
                        write_json_line(&summary)?;
                    }
                    Ok(())
                }
                OutputFormat::Pretty | OutputFormat::Text => {
                    for summary in summaries {
                        let endpoint = summary
                            .endpoint
                            .as_ref()
                            .map(|endpoint| format!("{}:{}", endpoint.host, endpoint.port))
                            .unwrap_or_else(|| "outside_warp_disabled".to_owned());
                        println!(
                            "{}\tpid={}\t{}\t{}",
                            summary.instance_id, summary.pid, summary.channel, endpoint
                        );
                    }
                    Ok(())
                }
            }
        }
    }
}

pub(super) fn run_app_command(
    command: AppCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        AppCommand::Ping(args) => run_action(args, ActionKind::AppPing, json!({}), output_format),
        AppCommand::Version(args) => {
            run_action(args, ActionKind::AppVersion, json!({}), output_format)
        }
    }
}

pub(super) fn run_tab_command(
    command: TabCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        TabCommand::Create(args) => {
            run_action(args, ActionKind::TabCreate, json!({}), output_format)
        }
    }
}

pub(super) fn run_drive_command(
    command: DriveCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        DriveCommand::Object(command) => match command {
            DriveObjectCommand::Create(args) => {
                let params = serde_json::to_value(DriveCreateParams {
                    object_type: args.object_type.into(),
                    name: args.name,
                    content: parse_drive_content(&args.content),
                })
                .map_err(encode_params_error)?;
                run_action(
                    args.target,
                    ActionKind::DriveObjectCreate,
                    params,
                    output_format,
                )
            }
            DriveObjectCommand::Update(args) => {
                let (target_args, action, params, target) =
                    drive_content_request(args, ActionKind::DriveObjectUpdate)?;
                run_action_with_target(target_args, action, params, target, output_format)
            }
            DriveObjectCommand::Delete(args) => {
                let (target_args, action, params, target) =
                    drive_id_request(args, ActionKind::DriveObjectDelete)?;
                run_action_with_target(target_args, action, params, target, output_format)
            }
            DriveObjectCommand::Insert(args) => {
                let (target_args, action, params, target) =
                    drive_id_request(args, ActionKind::DriveObjectInsert)?;
                run_action_with_target(target_args, action, params, target, output_format)
            }
            DriveObjectCommand::ShareToTeam(args) => {
                let (target_args, action, params, target) =
                    drive_id_request(args, ActionKind::DriveObjectShareToTeam)?;
                run_action_with_target(target_args, action, params, target, output_format)
            }
        },
    }
}

fn drive_content_request(
    args: DriveObjectContentArgs,
    action: ActionKind,
) -> Result<(TargetArgs, ActionKind, serde_json::Value, TargetSelector), ControlError> {
    let object_type: DriveObjectType = args.object_type.into();
    let target = drive_id_target(object_type, &args.id);
    let params = serde_json::to_value(DriveUpdateParams {
        object_type,
        id: args.id,
        content: parse_drive_content(&args.content),
    })
    .map_err(encode_params_error)?;
    Ok((args.target, action, params, target))
}

fn drive_id_request(
    args: DriveObjectIdArgs,
    action: ActionKind,
) -> Result<(TargetArgs, ActionKind, serde_json::Value, TargetSelector), ControlError> {
    let object_type: DriveObjectType = args.object_type.into();
    let target = drive_id_target(object_type, &args.id);
    let params = match action {
        ActionKind::DriveObjectDelete => serde_json::to_value(DriveDeleteParams {
            object_type,
            id: args.id,
        }),
        ActionKind::DriveObjectInsert => serde_json::to_value(DriveInsertParams {
            object_type,
            id: args.id,
        }),
        ActionKind::DriveObjectShareToTeam => serde_json::to_value(DriveShareToTeamParams {
            object_type,
            id: args.id,
        }),
        _ => {
            return Err(ControlError::new(
                ErrorCode::UnsupportedAction,
                format!("{} is not a Drive object id mutation", action.as_str()),
            ));
        }
    }
    .map_err(encode_params_error)?;
    Ok((args.target, action, params, target))
}

fn drive_id_target(object_type: DriveObjectType, id: &str) -> TargetSelector {
    TargetSelector {
        drive: Some(DriveTarget::Id {
            object_type,
            id: DriveObjectSelector(id.to_owned()),
        }),
        ..Default::default()
    }
}

fn parse_drive_content(content: &str) -> serde_json::Value {
    serde_json::from_str(content).unwrap_or_else(|_| serde_json::Value::String(content.to_owned()))
}

fn encode_params_error(err: serde_json::Error) -> ControlError {
    ControlError::with_details(
        ErrorCode::InvalidParams,
        "failed to encode Drive object parameters",
        err.to_string(),
    )
}

fn run_action(
    args: TargetArgs,
    action: ActionKind,
    params: serde_json::Value,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    run_action_with_target(
        args,
        action,
        params,
        TargetSelector::default(),
        output_format,
    )
}

fn run_action_with_target(
    args: TargetArgs,
    action: ActionKind,
    params: serde_json::Value,
    target: TargetSelector,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    let records = local_control::discovery::list_instances();
    let selector = instance_selector(args);
    let instance = select_instance(&records, &selector)?;
    let mut request = RequestEnvelope::new(Action {
        kind: action,
        params,
    });
    request.target = target;
    let response = local_control::client::send_request(&instance, &request)?;
    let local_control::protocol::ControlResponse::Ok { data } = response.response else {
        return Err(ControlError::new(
            ErrorCode::Internal,
            "local-control request failed without an error payload",
        ));
    };
    match output_format {
        OutputFormat::Json => write_json(&data),
        OutputFormat::Ndjson => write_json_line(&data),
        OutputFormat::Pretty | OutputFormat::Text => write_json(&data),
    }
}
