use std::os::fd::OwnedFd;

use anyhow::{anyhow, bail, Context, Result};
use ashpd::desktop::{
    screencast::{
        CursorMode, Screencast, SelectSourcesOptions, SourceType,
        Stream as ScreencastStream,
    },
    PersistMode,
};
use pipewire as pw;
use pw::{properties::properties, spa};

const SAMPLE_X: usize = 100;
const SAMPLE_Y: usize = 100;

struct UserData {
    format: spa::param::video::VideoInfoRaw,
}

#[tokio::main]
async fn main() -> Result<()> {
    let (stream, fd) = open_portal().await?;
    let node_id = stream.pipe_wire_node_id();

    eprintln!("PipeWire node id: {node_id}");
    start_pipewire(node_id, fd)?;
    Ok(())
}

async fn open_portal() -> Result<(ScreencastStream, OwnedFd)> {
    let proxy = Screencast::new().await?;
    let session = proxy.create_session(Default::default()).await?;

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

    let response = proxy.start(&session, None, Default::default()).await?.response()?;
    let stream = response
        .streams()
        .first()
        .ok_or_else(|| anyhow!("no stream selected"))?
        .to_owned();

    let fd = proxy.open_pipe_wire_remote(&session, Default::default()).await?;
    Ok((stream, fd))
}

fn start_pipewire(node_id: u32, fd: OwnedFd) -> Result<()> {
    pw::init();

    let mainloop = pw::main_loop::MainLoopBox::new(None)?;
    let context = pw::context::ContextBox::new(mainloop.loop_(), None)?;
    let core = context.connect_fd(fd, None)?;

    let stream = pw::stream::StreamBox::new(
        &core,
        "pixel-probe",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )?;

    let data = UserData {
        format: Default::default(),
    };

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .param_changed(|_, user_data, id, param| {
            let Some(param) = param else { return };
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }

            let Ok((media_type, media_subtype)) = spa::param::format_utils::parse_format(param)
            else {
                return;
            };

            if media_type != spa::param::format::MediaType::Video
                || media_subtype != spa::param::format::MediaSubtype::Raw
            {
                return;
            }

            user_data.format.parse(param).expect("parse video format");

            let size = user_data.format.size();
            eprintln!(
                "format={:?}, size={}x{}",
                user_data.format.format(),
                size.width,
                size.height
            );
        })
        .process(|stream, user_data| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };

            let datas = buffer.datas_mut();
            let Some(data) = datas.first_mut() else {
                return;
            };

            let (offset, size, stride) = {
                let chunk = data.chunk();
                (
                    chunk.offset() as usize,
                    chunk.size() as usize,
                    chunk.stride(),
                )
            };

            let Some(bytes) = data.data() else {
                return;
            };

            let width = user_data.format.size().width as usize;
            let height = user_data.format.size().height as usize;
            let format = user_data.format.format();

            match sample_pixel(bytes, offset, size, stride, width, height, format, SAMPLE_X, SAMPLE_Y)
            {
                Some([r, g, b, a]) => println!("rgba({r}, {g}, {b}, {a})"),
                None => eprintln!("could not sample pixel"),
            }
        })
        .register()?;

    let mut params = [make_format_param()?];

    stream.connect(
        spa::utils::Direction::Input,
        Some(node_id),
        pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;

    mainloop.run();
    Ok(())
}

fn make_format_param() -> Result<&'static spa::pod::Pod> {
    let obj = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaType,
            Id,
            spa::param::format::MediaType::Video
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaSubtype,
            Id,
            spa::param::format::MediaSubtype::Raw
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            spa::param::video::VideoFormat::RGBA,
            spa::param::video::VideoFormat::RGBA,
            spa::param::video::VideoFormat::RGBx,
            spa::param::video::VideoFormat::BGRx,
            spa::param::video::VideoFormat::RGB
        )
    );

    let bytes = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(obj),
    )?
        .0
        .into_inner();

    // PipeWire wants the Pod reference to live until after connect().
    // For a small prototype this leak is acceptable; wrap this better in real code.
    let bytes = Box::leak(bytes.into_boxed_slice());
    Ok(spa::pod::Pod::from_bytes(bytes).context("Could not convert bytes to Pod")?)
}

#[allow(clippy::too_many_arguments)]
fn sample_pixel(
    bytes: &[u8],
    offset: usize,
    size: usize,
    stride: i32,
    width: usize,
    height: usize,
    format: spa::param::video::VideoFormat,
    x: usize,
    y: usize,
) -> Option<[u8; 4]> {
    if x >= width || y >= height {
        return None;
    }

    let bpp = match format {
        spa::param::video::VideoFormat::RGB => 3,
        spa::param::video::VideoFormat::RGBA
        | spa::param::video::VideoFormat::RGBx
        | spa::param::video::VideoFormat::BGRx => 4,
        _ => return None,
    };

    let row_stride = if stride == 0 {
        width * bpp
    } else {
        stride.unsigned_abs() as usize
    };

    let row = if stride < 0 { height - 1 - y } else { y };
    let idx = offset + row * row_stride + x * bpp;
    let valid_end = offset.saturating_add(size).min(bytes.len());
    let px = bytes.get(idx..idx + bpp).filter(|_| idx + bpp <= valid_end)?;

    match format {
        spa::param::video::VideoFormat::RGB => Some([px[0], px[1], px[2], 255]),
        spa::param::video::VideoFormat::RGBA => Some([px[0], px[1], px[2], px[3]]),
        spa::param::video::VideoFormat::RGBx => Some([px[0], px[1], px[2], 255]),
        spa::param::video::VideoFormat::BGRx => Some([px[2], px[1], px[0], 255]),
        _ => None,
    }
}