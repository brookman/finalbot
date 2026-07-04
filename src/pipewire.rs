//! `PipeWire` stream setup, format negotiation, and main-loop signal handling.
//!
//! [`start`] opens a `PipeWire` stream connected to the given screencast node
//! and samples a single pixel on every incoming buffer.

use std::os::fd::OwnedFd;
use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use pipewire::{
    context::ContextBox,
    keys,
    main_loop::{MainLoop, MainLoopBox},
    properties::properties,
    spa,
    spa::param::{
        format::{FormatProperties, MediaSubtype, MediaType},
        format_utils,
        video::{VideoFormat, VideoInfoRaw},
        ParamType,
    },
    spa::pod::{serialize::PodSerializer, Pod, Value},
    spa::utils::Direction,
    stream::{StreamBox, StreamFlags},
};

use crate::pixel::BufferContext;

static MAIN_LOOP: OnceLock<&'static MainLoop> = OnceLock::new();

struct UserData {
    format: VideoInfoRaw,
}

pub fn start(node_id: u32, fd: OwnedFd, sample_x: u32, sample_y: u32) -> Result<()> {
    pipewire::init();

    let main_loop = MainLoopBox::new(None)?;
    let main_loop: &'static MainLoop = Box::leak(Box::new(main_loop));
    if MAIN_LOOP.set(main_loop).is_err() {
        eprintln!("MAIN_LOOP already set — continuing");
    }

    ctrlc::set_handler(move || {
        if let Some(ml) = MAIN_LOOP.get() {
            ml.quit();
        }
    })?;

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

    let format_param_bytes = build_format_param_bytes()?;
    let format_param = Pod::from_bytes(&format_param_bytes)
        .ok_or_else(|| anyhow!("Could not convert bytes to Pod"))?;

    let data = UserData {
        format: VideoInfoRaw::default(),
    };

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .param_changed(|_, user_data, id, param| {
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
                eprintln!("Failed to parse video format: {e}");
                return;
            }

            let size = user_data.format.size();
            eprintln!(
                "format={:?}, size={}x{}",
                user_data.format.format(),
                size.width,
                size.height
            );
        })
        .process(move |stream, user_data| {
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

            let ctx = BufferContext {
                offset,
                stride,
                width: user_data.format.size().width,
                height: user_data.format.size().height,
                format: user_data.format.format(),
            };

            match ctx.sample_pixel(bytes, sample_x, sample_y) {
                Some(pixel) => println!(
                    "rgba({}, {}, {}, {})",
                    pixel[0], pixel[1], pixel[2], pixel[3]
                ),
                None => eprintln!("could not sample pixel"),
            }
        })
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

fn build_format_param_bytes() -> Result<Vec<u8>> {
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
        .map_err(Into::into)
}
