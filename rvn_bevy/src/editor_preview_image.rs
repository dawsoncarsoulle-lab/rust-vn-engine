//! Copy the real Camera2d render target after the existing camera graph.
use crate::editor_preview::{Output, RenderScope,RenderInputFrame,RenderDebugger,RenderInspection,RenderStartupSource};
use bevy::prelude::*;
use bevy::render::{
    render_asset::RenderAssets,
    render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
    render_resource::{
        Buffer, BufferDescriptor, BufferUsages, Extent3d, ImageCopyBuffer, ImageDataLayout,
        Maintain, MapMode,
    },
    renderer::{RenderContext, RenderDevice},
    texture::GpuImage,
    Extract, ExtractSchedule, Render, RenderApp, RenderSet,
};
use rvn_core::editor_preview::{FrameScope, Header, Packet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Resource)]
pub(crate) struct Target {
    pub image: Handle<Image>,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Resource)]
struct Capture {
    target: Target,
    scope: FrameScope,
    debugger:Option<String>,
    inspection:Option<Arc<str>>,
    input: Option<Arc<crate::editor_preview_input::PhysicalFrame>>,
    output: Output,
    startup:Option<Arc<rvn_core::engine::StartupObservationSource>>,
}
struct Slot {
    buffer: Buffer,
    busy: Arc<AtomicBool>,
    pending: Option<Capture>,
}
#[derive(Resource, Default)]
struct Copies(Mutex<Vec<Slot>>);
#[derive(Debug, Clone, PartialEq, Eq, Hash, RenderLabel)]
struct CopyAfterCameras;
struct CopyNode;
pub(crate) struct ImagePlugin;
impl Plugin for ImagePlugin {
    fn build(&self, app: &mut App) {
        let render = app.sub_app_mut(RenderApp);
        let mut graph = render.world_mut().resource_mut::<RenderGraph>();
        graph.add_node(CopyAfterCameras, CopyNode);
        graph.add_node_edge(bevy::render::graph::CameraDriverLabel, CopyAfterCameras);
        render
            .init_resource::<Copies>()
            .add_systems(ExtractSchedule, extract)
            .add_systems(Render, map_copies.after(RenderSet::Render));
    }
}
fn extract(
    mut commands: Commands,
    target: Extract<Option<Res<Target>>>,
    scope: Extract<Option<Res<RenderScope>>>,
    output: Extract<Res<Output>>,
    input: Extract<Option<Res<RenderInputFrame>>>,
    debugger:Extract<Option<Res<RenderDebugger>>>,
    inspection:Extract<Option<Res<RenderInspection>>>,
    startup:Extract<Option<Res<RenderStartupSource>>>,
) {
    if let (Some(target), Some(scope)) = (target.as_ref(), scope.as_ref()) {
        crate::editor_preview::trace_once(3,format_args!("Extract actual target={}x{} instance={} epoch={} sequence={}",target.width,target.height,scope.0.instance,scope.0.epoch,scope.0.sequence));
        commands.insert_resource(Capture {
            startup:startup.as_ref().and_then(|source|source.0.clone()),
            target: (**target).clone(),
            scope: scope.0.clone(),
            debugger:debugger.as_ref().and_then(|snapshot|snapshot.0.clone()),
            inspection:inspection.as_ref().and_then(|snapshot|snapshot.0.clone()),
            input: input.as_ref().and_then(|input|input.0.clone()),
            output: (**output).clone(),
        });
    } else {
        crate::editor_preview::trace_once(3,format_args!("Extract target_present={} scope_present={}",target.is_some(),scope.is_some()));
        commands.remove_resource::<Capture>();
    }
}
fn stride(width: u32) -> usize {
    RenderDevice::align_copy_bytes_per_row(width as usize * 4)
}
fn unpad(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let count = rvn_core::editor_preview::image_bytes(width, height)?;
    let row = width as usize * 4;
    let padded = stride(width);
    if bytes.len() != padded * height as usize {
        return Err("GPU copy has an unexpected padded length".into());
    }
    let mut output = Vec::with_capacity(count);
    for y in 0..height as usize {
        output.extend_from_slice(&bytes[y * padded..y * padded + row]);
    }
    Ok(output)
}
impl Node for CopyNode {
    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let Some(capture) = world.get_resource::<Capture>() else {
            return Ok(());
        };
        let gpu = world.resource::<RenderAssets<GpuImage>>();
        let Some(image) = gpu.get(&capture.target.image) else {
            return Ok(());
        };
        if image.size.x != capture.target.width || image.size.y != capture.target.height {
            capture.output.failed();
            return Ok(());
        }
        let Ok(mut slots) = world.resource::<Copies>().0.lock() else {
            capture.output.failed();
            return Ok(());
        };
        if slots.is_empty() {
            let size = stride(capture.target.width) * capture.target.height as usize;
            for _ in 0..2 {
                slots.push(Slot {
                    buffer: render.render_device().create_buffer(&BufferDescriptor {
                        label: Some("Editor actual-frame readback"),
                        size: size as u64,
                        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    busy: Arc::new(AtomicBool::new(false)),
                    pending: None,
                });
            }
        }
        let Some(slot) = slots
            .iter_mut()
            .find(|slot| !slot.busy.load(Ordering::Acquire))
        else {
            return Ok(());
        };
        slot.busy.store(true, Ordering::Release);
        slot.pending = Some(capture.clone());
        // The camera commands and this copy share RenderContext submission order.
        // Do not submit an independent queue buffer before the camera encoder.
        render.command_encoder().copy_texture_to_buffer(
            image.texture.as_image_copy(),
            ImageCopyBuffer {
                buffer: &slot.buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(stride(capture.target.width) as u32),
                    rows_per_image: None,
                },
            },
            Extent3d {
                width: capture.target.width,
                height: capture.target.height,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }
}
fn map_copies(copies: Res<Copies>, device: Res<RenderDevice>) {
    let Ok(mut slots) = copies.0.lock() else {
        return;
    };
    for slot in slots.iter_mut() {
        let Some(capture) = slot.pending.take() else {
            continue;
        };
        let buffer = slot.buffer.clone();
        let busy = slot.busy.clone();
        slot.buffer
            .slice(..)
            .map_async(MapMode::Read, move |result| {
                crate::editor_preview::trace_once(4,format_args!("GPU map callback success={} target={}x{} instance={} epoch={} sequence={}",result.is_ok(),capture.target.width,capture.target.height,capture.scope.instance,capture.scope.epoch,capture.scope.sequence));
                if result.is_err() {
                    capture.output.failed();
                    busy.store(false, Ordering::Release);
                    return;
                }
                let completed=(capture.scope.instance,capture.scope.epoch,capture.scope.sequence);
                let mapped = buffer.slice(..).get_mapped_range();
                let pixels = unpad(&mapped, capture.target.width, capture.target.height);
                let unpad_succeeded=pixels.is_ok();
                drop(mapped);
                buffer.unmap();
                match pixels {
                    Ok(pixels) => capture.output.frame_with_startup(Packet {
                        header: Header::Frame {
                            scope: capture.scope,
                            width: capture.target.width,
                            height: capture.target.height,
                            debugger:capture.debugger,
                            inspection:capture.inspection.map(|snapshot|snapshot.to_string()),
                        },
                        pixels,
                    },capture.input,capture.startup),
                    Err(_) => capture.output.failed(),
                }
                busy.store(false, Ordering::Release);
                crate::editor_preview::trace_once(8,format_args!("Actual GPU map callback finished unpad_success={} instance={} epoch={} sequence={}",unpad_succeeded,completed.0,completed.1,completed.2));
            });
    }
    drop(slots);
    // Nonblocking; no unbounded GPU wait or channel of images on the editor thread.
    device.poll(Maintain::Poll);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn row_alignment_keeps_all_three_pixels_and_never_copies_padding() {
        assert_eq!(stride(3), 256);
        let mut bytes = vec![99; 512];
        bytes[..12].copy_from_slice(&[1; 12]);
        bytes[256..268].copy_from_slice(&[2; 12]);
        assert_eq!(
            unpad(&bytes, 3, 2).unwrap(),
            [vec![1; 12], vec![2; 12]].concat()
        );
        assert!(unpad(&bytes[..511], 3, 2).is_err());
    }
}
