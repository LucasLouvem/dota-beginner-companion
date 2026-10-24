//! Testes unitários de `src/presentation/pages/diagnostics.rs`.

use super::*;
use tokio::sync::mpsc;

#[test]
fn sends_command_while_core_is_running() {
    let (commands, mut command_receiver) = mpsc::unbounded_channel();

    send_command(&commands, UiCommand::TestConnection).unwrap();

    assert!(matches!(
        command_receiver.try_recv(),
        Ok(UiCommand::TestConnection)
    ));
}

#[test]
fn returns_rejected_command_when_core_stopped() {
    let (commands, command_receiver) = mpsc::unbounded_channel();
    drop(command_receiver);

    let result = send_command(
        &commands,
        UiCommand::InstallGsiConfig {
            install_root: PathBuf::from("/lib"),
            overwrite: true,
        },
    );

    let Err(CoreConnectionError::CommandRejected {
        command:
            UiCommand::InstallGsiConfig {
                install_root,
                overwrite,
            },
    }) = result
    else {
        panic!("esperava comando recusado: {result:?}");
    };
    assert_eq!(install_root, PathBuf::from("/lib"));
    assert!(overwrite);
}
