use std::io::Write as _;

use anyhow::Context as _;
use clap::{Args, Parser, Subcommand};
use local_control::protocol::{Action, ActionKind, ControlResponse, RequestEnvelope};
use local_control::selection::{InstanceSelector, select_instance};
use serde::Serialize;
use serde_json::json;

use crate::agent::OutputFormat;

#[derive(Debug, Parser)]
#[command(
    name = "warpctrl",
    display_name = "warpctrl",
    about = "Control a running local Warp app instance"
)]
pub struct ControlArgs {
    /// Set the output format.
    #[arg(
        long = "output-format",
        global = true,
        value_enum,
        default_value_t = OutputFormat::Pretty,
        env = "WARP_OUTPUT_FORMAT"
    )]
    pub output_format: OutputFormat,

    #[command(subcommand)]
    pub command: ControlCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ControlCommand {
    /// Control a running local Warp app instance.
    #[command(subcommand)]
    App(AppCommand),

    /// Inspect local Warp app instances.
    #[command(subcommand)]
    Instance(InstanceCommand),

    /// Control local Warp windows.
    #[command(subcommand)]
    Window(WindowCommand),

    /// Control local Warp tabs.
    #[command(subcommand)]
    Tab(TabCommand),

    /// Control local Warp panes.
    #[command(subcommand)]
    Pane(PaneCommand),

    /// Inspect local Warp sessions.
    #[command(subcommand)]
    Session(SessionCommand),

    /// Control local Warp input buffers.
    #[command(subcommand)]
    Input(InputCommand),

    /// Inspect or update the active theme.
    #[command(subcommand)]
    Theme(ThemeCommand),

    /// Adjust font size in a running Warp app.
    #[command(subcommand, name = "font-size")]
    FontSize(FontSizeCommand),

    /// Adjust UI zoom in a running Warp app.
    #[command(subcommand)]
    Zoom(ZoomCommand),

    /// Read or mutate allowlisted Warp settings.
    #[command(subcommand)]
    Setting(SettingCommand),
}

#[derive(Debug, Clone, Subcommand)]
pub enum AppCommand {
    /// Check that a local Warp instance accepts authenticated control requests.
    Ping(TargetArgs),
    /// Print a small snapshot of live app state from a local Warp instance.
    Inspect(TargetArgs),
    /// Print local-control and app version metadata.
    Version(TargetArgs),
    /// Print the active-instance summary exposed by the local bridge.
    Active(TargetArgs),
    /// Open settings when the app bridge supports the mutation.
    SettingsOpen(TargetArgs),
    /// Open the command palette when the app bridge supports the mutation.
    CommandPaletteOpen(QueryTargetArgs),
    /// Open command search when the app bridge supports the mutation.
    CommandSearchOpen(QueryTargetArgs),
    /// Open Warp Drive when the app bridge supports the mutation.
    WarpDriveOpen(TargetArgs),
    /// Toggle Warp Drive when the app bridge supports the mutation.
    WarpDriveToggle(TargetArgs),
    /// Toggle Resource Center when the app bridge supports the mutation.
    ResourceCenterToggle(TargetArgs),
    /// Toggle the AI assistant panel when the app bridge supports the mutation.
    AiAssistantToggle(TargetArgs),
    /// Toggle the code review panel when the app bridge supports the mutation.
    CodeReviewToggle(TargetArgs),
    /// Toggle vertical tabs when the app bridge supports the mutation.
    VerticalTabsToggle(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum InstanceCommand {
    /// List locally discoverable Warp instances.
    List,
}

#[derive(Debug, Clone, Subcommand)]
pub enum WindowCommand {
    List(TargetArgs),
    Create(TargetArgs),
    Focus(TargetArgs),
    Close(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum TabCommand {
    List(TargetArgs),
    Create(TargetArgs),
    Activate(TargetArgs),
    Move(DirectionTargetArgs),
    Rename(TextTargetArgs),
    Close(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum PaneCommand {
    List(TargetArgs),
    Split(DirectionTargetArgs),
    Focus(TargetArgs),
    Navigate(DirectionTargetArgs),
    Close(TargetArgs),
    Maximize(TargetArgs),
    Resize(DirectionTargetArgs),
    SessionPrevious(TargetArgs),
    SessionNext(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SessionCommand {
    List(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum InputCommand {
    Insert(TextTargetArgs),
    Replace(TextTargetArgs),
    Clear(TargetArgs),
    ModeSet(ModeTargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ThemeCommand {
    List(TargetArgs),
    Set(TextTargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum FontSizeCommand {
    Increase(TargetArgs),
    Decrease(TargetArgs),
    Reset(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ZoomCommand {
    Increase(TargetArgs),
    Decrease(TargetArgs),
    Reset(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SettingCommand {
    List(TargetArgs),
    Get(SettingKeyArgs),
    Set(SettingValueArgs),
    Toggle(SettingKeyArgs),
}

#[derive(Debug, Clone, Args, Default)]
pub struct TargetArgs {
    /// Target a specific local Warp instance id from `warp instance list`.
    #[arg(long = "instance")]
    pub instance: Option<String>,

    /// Target a specific local Warp process id.
    #[arg(long = "pid", conflicts_with = "instance")]
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct QueryTargetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct DirectionTargetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long)]
    pub direction: String,
}

#[derive(Debug, Clone, Args)]
pub struct ModeTargetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub mode: String,
}

#[derive(Debug, Clone, Args)]
pub struct TextTargetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub value: String,
}

#[derive(Debug, Clone, Args)]
pub struct SettingKeyArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub key: String,
}

#[derive(Debug, Clone, Args)]
pub struct SettingValueArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub key: String,
    pub value: String,
}

#[derive(Serialize)]
struct InstanceSummary {
    instance_id: String,
    pid: u32,
    channel: String,
    app_id: String,
    app_version: Option<String>,
    started_at: String,
    endpoint: local_control::discovery::ControlEndpoint,
    actions: Vec<String>,
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
            actions: record
                .actions
                .into_iter()
                .map(|metadata| metadata.name)
                .collect(),
        }
    }
}

pub fn run(args: ControlArgs) -> anyhow::Result<()> {
    let output_format = args.output_format;
    match args.command {
        ControlCommand::App(command) => run_app_command(command, output_format),
        ControlCommand::Instance(command) => run_instance_command(command, output_format),
        ControlCommand::Window(command) => run_window_command(command, output_format),
        ControlCommand::Tab(command) => run_tab_command(command, output_format),
        ControlCommand::Pane(command) => run_pane_command(command, output_format),
        ControlCommand::Session(command) => run_session_command(command, output_format),
        ControlCommand::Input(command) => run_input_command(command, output_format),
        ControlCommand::Theme(command) => run_theme_command(command, output_format),
        ControlCommand::FontSize(command) => run_font_size_command(command, output_format),
        ControlCommand::Zoom(command) => run_zoom_command(command, output_format),
        ControlCommand::Setting(command) => run_setting_command(command, output_format),
    }
}

fn run_app_command(command: AppCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    match command {
        AppCommand::Ping(args) => run_action(args, ActionKind::AppPing, json!({}), output_format),
        AppCommand::Inspect(args) => {
            run_action(args, ActionKind::AppInspect, json!({}), output_format)
        }
        AppCommand::Version(args) => {
            run_action(args, ActionKind::AppVersion, json!({}), output_format)
        }
        AppCommand::Active(args) => {
            run_action(args, ActionKind::AppActive, json!({}), output_format)
        }
        AppCommand::SettingsOpen(args) => {
            run_action(args, ActionKind::AppSettingsOpen, json!({}), output_format)
        }
        AppCommand::CommandPaletteOpen(args) => {
            run_query_action(args, ActionKind::AppCommandPaletteOpen, output_format)
        }
        AppCommand::CommandSearchOpen(args) => {
            run_query_action(args, ActionKind::AppCommandSearchOpen, output_format)
        }
        AppCommand::WarpDriveOpen(args) => {
            run_action(args, ActionKind::AppWarpDriveOpen, json!({}), output_format)
        }
        AppCommand::WarpDriveToggle(args) => run_action(
            args,
            ActionKind::AppWarpDriveToggle,
            json!({}),
            output_format,
        ),
        AppCommand::ResourceCenterToggle(args) => run_action(
            args,
            ActionKind::AppResourceCenterToggle,
            json!({}),
            output_format,
        ),
        AppCommand::AiAssistantToggle(args) => run_action(
            args,
            ActionKind::AppAiAssistantToggle,
            json!({}),
            output_format,
        ),
        AppCommand::CodeReviewToggle(args) => run_action(
            args,
            ActionKind::AppCodeReviewToggle,
            json!({}),
            output_format,
        ),
        AppCommand::VerticalTabsToggle(args) => run_action(
            args,
            ActionKind::AppVerticalTabsToggle,
            json!({}),
            output_format,
        ),
    }
}

fn run_instance_command(
    command: InstanceCommand,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
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
                        println!(
                            "{}\tpid={}\t{}\t{}:{}",
                            summary.instance_id,
                            summary.pid,
                            summary.channel,
                            summary.endpoint.host,
                            summary.endpoint.port
                        );
                    }
                    Ok(())
                }
            }
        }
    }
}

fn run_window_command(command: WindowCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    match command {
        WindowCommand::List(args) => {
            run_action(args, ActionKind::WindowList, json!({}), output_format)
        }
        WindowCommand::Create(args) => {
            run_action(args, ActionKind::WindowCreate, json!({}), output_format)
        }
        WindowCommand::Focus(args) => {
            run_action(args, ActionKind::WindowFocus, json!({}), output_format)
        }
        WindowCommand::Close(args) => {
            run_action(args, ActionKind::WindowClose, json!({}), output_format)
        }
    }
}

fn run_tab_command(command: TabCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    match command {
        TabCommand::List(args) => run_action(args, ActionKind::TabList, json!({}), output_format),
        TabCommand::Create(args) => {
            run_action(args, ActionKind::TabCreate, json!({}), output_format)
        }
        TabCommand::Activate(args) => {
            run_action(args, ActionKind::TabActivate, json!({}), output_format)
        }
        TabCommand::Move(args) => run_direction_action(args, ActionKind::TabMove, output_format),
        TabCommand::Rename(args) => run_text_action(args, ActionKind::TabRename, output_format),
        TabCommand::Close(args) => run_action(args, ActionKind::TabClose, json!({}), output_format),
    }
}

fn run_pane_command(command: PaneCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    match command {
        PaneCommand::List(args) => run_action(args, ActionKind::PaneList, json!({}), output_format),
        PaneCommand::Split(args) => {
            run_direction_action(args, ActionKind::PaneSplit, output_format)
        }
        PaneCommand::Focus(args) => {
            run_action(args, ActionKind::PaneFocus, json!({}), output_format)
        }
        PaneCommand::Navigate(args) => {
            run_direction_action(args, ActionKind::PaneNavigate, output_format)
        }
        PaneCommand::Close(args) => {
            run_action(args, ActionKind::PaneClose, json!({}), output_format)
        }
        PaneCommand::Maximize(args) => {
            run_action(args, ActionKind::PaneMaximize, json!({}), output_format)
        }
        PaneCommand::Resize(args) => {
            run_direction_action(args, ActionKind::PaneResize, output_format)
        }
        PaneCommand::SessionPrevious(args) => run_action(
            args,
            ActionKind::PaneSessionPrevious,
            json!({}),
            output_format,
        ),
        PaneCommand::SessionNext(args) => {
            run_action(args, ActionKind::PaneSessionNext, json!({}), output_format)
        }
    }
}

fn run_session_command(command: SessionCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    match command {
        SessionCommand::List(args) => {
            run_action(args, ActionKind::SessionList, json!({}), output_format)
        }
    }
}

fn run_input_command(command: InputCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    match command {
        InputCommand::Insert(args) => run_text_action(args, ActionKind::InputInsert, output_format),
        InputCommand::Replace(args) => {
            run_text_action(args, ActionKind::InputReplace, output_format)
        }
        InputCommand::Clear(args) => {
            run_action(args, ActionKind::InputClear, json!({}), output_format)
        }
        InputCommand::ModeSet(args) => run_action(
            args.target,
            ActionKind::InputModeSet,
            json!({ "mode": args.mode }),
            output_format,
        ),
    }
}

fn run_theme_command(command: ThemeCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    match command {
        ThemeCommand::List(args) => {
            run_action(args, ActionKind::ThemeList, json!({}), output_format)
        }
        ThemeCommand::Set(args) => run_text_action(args, ActionKind::ThemeSet, output_format),
    }
}

fn run_font_size_command(
    command: FontSizeCommand,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    let (args, operation) = match command {
        FontSizeCommand::Increase(args) => (args, "increase"),
        FontSizeCommand::Decrease(args) => (args, "decrease"),
        FontSizeCommand::Reset(args) => (args, "reset"),
    };
    run_action(
        args,
        ActionKind::AppearanceFontSize,
        json!({ "operation": operation }),
        output_format,
    )
}

fn run_zoom_command(command: ZoomCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    let (args, operation) = match command {
        ZoomCommand::Increase(args) => (args, "increase"),
        ZoomCommand::Decrease(args) => (args, "decrease"),
        ZoomCommand::Reset(args) => (args, "reset"),
    };
    run_action(
        args,
        ActionKind::AppearanceZoom,
        json!({ "operation": operation }),
        output_format,
    )
}

fn run_setting_command(command: SettingCommand, output_format: OutputFormat) -> anyhow::Result<()> {
    match command {
        SettingCommand::List(args) => {
            run_action(args, ActionKind::SettingList, json!({}), output_format)
        }
        SettingCommand::Get(args) => {
            run_setting_key_action(args, ActionKind::SettingGet, output_format)
        }
        SettingCommand::Set(args) => {
            run_setting_value_action(args, ActionKind::SettingSet, output_format)
        }
        SettingCommand::Toggle(args) => {
            run_setting_key_action(args, ActionKind::SettingToggle, output_format)
        }
    }
}

fn run_query_action(
    args: QueryTargetArgs,
    action: ActionKind,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    run_action(
        args.target,
        action,
        json!({ "query": args.query }),
        output_format,
    )
}

fn run_direction_action(
    args: DirectionTargetArgs,
    action: ActionKind,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    run_action(
        args.target,
        action,
        json!({ "direction": args.direction }),
        output_format,
    )
}

fn run_text_action(
    args: TextTargetArgs,
    action: ActionKind,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    run_action(
        args.target,
        action,
        json!({ "value": args.value }),
        output_format,
    )
}

fn run_setting_key_action(
    args: SettingKeyArgs,
    action: ActionKind,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    run_action(
        args.target,
        action,
        json!({ "key": args.key }),
        output_format,
    )
}

fn run_setting_value_action(
    args: SettingValueArgs,
    action: ActionKind,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    run_action(
        args.target,
        action,
        json!({ "key": args.key, "value": args.value }),
        output_format,
    )
}

fn run_action(
    args: TargetArgs,
    action: ActionKind,
    params: serde_json::Value,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    let records = local_control::discovery::list_instances();
    let selector = instance_selector(args);
    let instance = select_instance(&records, &selector)?;
    let request = RequestEnvelope::new(Action {
        kind: action,
        params,
    });
    let response = local_control::client::send_request(&instance, &request)?;
    let ControlResponse::Ok { data } = response.response else {
        anyhow::bail!("local-control request failed without an error payload");
    };
    match output_format {
        OutputFormat::Json => write_json(&data),
        OutputFormat::Ndjson => write_json_line(&data),
        OutputFormat::Pretty | OutputFormat::Text => {
            if action == ActionKind::AppPing {
                println!("ok");
                Ok(())
            } else {
                write_json(&data).context("unable to print local-control data")
            }
        }
    }
}

fn instance_selector(args: TargetArgs) -> InstanceSelector {
    if let Some(instance_id) = args.instance {
        return InstanceSelector::Id(local_control::discovery::InstanceId(instance_id));
    }
    if let Some(pid) = args.pid {
        return InstanceSelector::Pid(pid);
    }
    InstanceSelector::Active
}

fn write_json(value: &impl Serialize) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer_pretty(&mut lock, value)?;
    writeln!(&mut lock)?;
    Ok(())
}

fn write_json_line(value: &impl Serialize) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value)?;
    writeln!(&mut lock)?;
    Ok(())
}
