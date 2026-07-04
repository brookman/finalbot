use std::os::fd::OwnedFd;

use anyhow::{anyhow, Result};
use ashpd::desktop::{
    screencast::{
        CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
        StartCastOptions, Stream as ScreencastStream,
    }, CreateSessionOptions,
    PersistMode,
};

pub async fn open_portal() -> Result<(ScreencastStream, OwnedFd)> {
    let proxy = Screencast::new().await?;
    let session = proxy
        .create_session(CreateSessionOptions::default())
        .await?;

    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Hidden)
                .set_sources(SourceType::Monitor | SourceType::Window)
                .set_multiple(false)
                .set_persist_mode(PersistMode::DoNot),
        )
        .await?;

    let response = proxy
        .start(&session, None, StartCastOptions::default())
        .await?
        .response()?;
    let stream = response
        .streams()
        .first()
        .ok_or_else(|| anyhow!("no stream selected"))?
        .to_owned();

    let fd = proxy
        .open_pipe_wire_remote(&session, OpenPipeWireRemoteOptions::default())
        .await?;
    Ok((stream, fd))
}
