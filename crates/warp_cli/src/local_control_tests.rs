use clap::Parser as _;

use super::*;

#[test]
fn parses_first_slice_tab_create() {
    let args = ControlArgs::try_parse_from(["warpctrl", "tab", "create", "--instance", "inst_123"])
        .expect("tab create parses");
    let ControlCommand::Tab(TabCommand::Create(target)) = args.command else {
        panic!("expected tab create command");
    };
    assert_eq!(target.instance.as_deref(), Some("inst_123"));
}

#[test]
fn parses_first_slice_instance_list() {
    let args = ControlArgs::try_parse_from(["warpctrl", "instance", "list"])
        .expect("instance list parses");
    assert!(matches!(
        args.command,
        ControlCommand::Instance(InstanceCommand::List)
    ));
}

#[test]
fn parses_first_slice_app_smoke_metadata_commands() {
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "ping"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "version"]).is_ok());
}

#[test]
fn rejects_future_catalog_commands_not_in_first_slice() {
    assert!(ControlArgs::try_parse_from(["warpctrl", "window", "list"]).is_err());
    assert!(ControlArgs::try_parse_from(["warpctrl", "tab", "list"]).is_err());
    assert!(ControlArgs::try_parse_from(["warpctrl", "setting", "list"]).is_err());
}
