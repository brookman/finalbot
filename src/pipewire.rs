//! `PipeWire` stream setup, format negotiation, and main-loop signal handling.
//!
//! [`start`] opens a `PipeWire` stream connected to the given screencast node
//! and calls a user-supplied callback on every incoming buffer.

use std::os::fd::OwnedFd;
use std::sync::OnceLock;
use std::sync::mpsc;

use crate::pixel::BufferContext;
use anyhow::{Result, anyhow};
use pipewire::{
    context::ContextBox,
    keys,
    main_loop::{MainLoop, MainLoopBox},
    properties::properties,
    spa,
    spa::param::{
        ParamType,
        format::{FormatProperties, MediaSubtype, MediaType},
        format_utils,
        video::{VideoFormat, VideoInfoRaw},
    },
    spa::pod::{Pod, Value, serialize::PodSerializer},
    spa::utils::Direction,
    stream::{Stream, StreamBox, StreamFlags},
};
use tracing::{error, info, warn};

static MAIN_LOOP: OnceLock<&'static MainLoop> = OnceLock::new();

struct UserData {
    format: VideoInfoRaw,
    tx: mpsc::Sender<(BufferContext, Vec<u8>)>,
}

fn init_pipewire() -> Result<&'static MainLoop> {
    pipewire::init();

    let main_loop = MainLoopBox::new(None)?;
    let main_loop: &'static MainLoop = Box::leak(Box::new(main_loop));
    if MAIN_LOOP.set(main_loop).is_err() {
        warn!("MAIN_LOOP already set — continuing");
    }

    ctrlc::set_handler(move || {
        if let Some(ml) = MAIN_LOOP.get() {
            ml.quit();
        }
    })?;

    Ok(main_loop)
}

fn on_format_changed(_: &Stream, user_data: &mut UserData, id: u32, param: Option<&Pod>) {
    let Some(param) = param else { return };
    if id != ParamType::Format.as_raw() {
        return;
    }

    let Ok((media_type, media_subtype)) = format_utils::parse_format(param) else {
        return;
    };

    if media_type != MediaType::Video || media_subtype != MediaSubtype::Raw {
        return;
    }

    if let Err(e) = user_data.format.parse(param) {
        error!("Failed to parse video format: {e}");
        return;
    }

    let size = user_data.format.size();
    info!(
        "format={:?}, size={}x{}",
        user_data.format.format(),
        size.width,
        size.height
    );
}

fn on_process_frame(stream: &Stream, user_data: &mut UserData) {
    let Some(mut buffer) = stream.dequeue_buffer() else {
        return;
    };

    let datas = buffer.datas_mut();
    let Some(data) = datas.first_mut() else {
        return;
    };

    let (offset, _, stride) = {
        let chunk = data.chunk();
        (
            usize::try_from(chunk.offset()).unwrap(),
            usize::try_from(chunk.size()).unwrap(),
            chunk.stride(),
        )
    };

    let Some(bytes) = data.data() else {
        return;
    };

    let height = user_data.format.size().height as usize;
    let stride_abs = stride.unsigned_abs() as usize;

    let min_needed = offset + stride_abs.saturating_mul(height);
    if bytes.len() < min_needed {
        error!("buffer too small: needed {min_needed}, got {}", bytes.len());
        return;
    }

    let ctx = BufferContext {
        offset,
        stride,
        width: user_data.format.size().width,
        height: user_data.format.size().height,
        format: user_data.format.format(),
    };

    let data = bytes.to_vec();
    let _ = user_data.tx.send((ctx, data));
}

pub fn start<F>(node_id: u32, fd: OwnedFd, mut on_frame: F) -> Result<()>
where
    F: FnMut(BufferContext, Vec<u8>) + Send + 'static,
{
    let main_loop = init_pipewire()?;

    let (tx, rx) = mpsc::channel::<(BufferContext, Vec<u8>)>();

    let _processor = std::thread::spawn(move || {
        while let Ok((ctx, data)) = rx.recv() {
            on_frame(ctx, data);
        }
    });

    let context = ContextBox::new(main_loop.loop_(), None)?;
    let core = context.connect_fd(fd, None)?;

    let stream = StreamBox::new(
        &core,
        "pixel-probe",
        properties! {
            *keys::MEDIA_TYPE => "Video",
            *keys::MEDIA_CATEGORY => "Capture",
            *keys::MEDIA_ROLE => "Screen",
        },
    )?;

    let format_param = build_format_param()?;

    let data = UserData {
        format: VideoInfoRaw::default(),
        tx,
    };

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .param_changed(on_format_changed)
        .process(on_process_frame)
        .register()?;

    let mut params = [format_param];
    stream.connect(
        Direction::Input,
        Some(node_id),
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;

    main_loop.run();
    Ok(())
}

fn build_format_param() -> Result<&'static Pod> {
    static FORMAT_BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    let bytes = FORMAT_BYTES.get_or_init(|| {
        let obj = spa::pod::object!(
            spa::utils::SpaTypes::ObjectParamFormat,
            ParamType::EnumFormat,
            spa::pod::property!(FormatProperties::MediaType, Id, MediaType::Video),
            spa::pod::property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
            spa::pod::property!(
                FormatProperties::VideoFormat,
                Choice,
                Enum,
                Id,
                VideoFormat::RGBA,
                VideoFormat::RGBA,
                VideoFormat::RGBx,
                VideoFormat::BGRx,
                VideoFormat::RGB
            )
        );
        PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(obj))
            .map(|(cursor, _)| cursor.into_inner())
            .expect("Format param serialization failed")
    });
    Pod::from_bytes(bytes).ok_or_else(|| anyhow!("Could not convert bytes to Pod"))
}
