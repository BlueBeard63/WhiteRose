//! Night light's GL pass on a real (or Mesa software) GPU: every pixel is
//! multiplied by the colour of the chosen temperature, and the renderer's own
//! blending still works afterwards.

use smithay::{
    backend::{
        allocator::Fourcc,
        egl::{EGLContext, EGLDevice, EGLDisplay},
        renderer::{
            gles::{GlesRenderer, GlesTexture},
            Bind, Color32F, ExportMem, Frame, Offscreen, Renderer,
        },
    },
    utils::{Buffer, Physical, Rectangle, Size, Transform},
};
use whiterose_wm::night_light::{color, Pass};

fn renderer() -> Option<GlesRenderer> {
    // Prefer a software device so the test behaves the same everywhere.
    let devices: Vec<EGLDevice> = EGLDevice::enumerate().ok()?.collect();
    let device = devices
        .iter()
        .find(|d| d.extensions().iter().any(|e| e == "EGL_MESA_device_software"))
        .or(devices.first())?
        .clone();
    let display = unsafe { EGLDisplay::new(device) }.ok()?;
    let context = EGLContext::new(&display).ok()?;
    unsafe { GlesRenderer::new(context) }.ok()
}

/// A mid grey, so multiplying (the right result) differs from replacing.
const BASE: f32 = 0.6;

fn pixel(renderer: &mut GlesRenderer, tint: Option<[f32; 3]>, then_draw_half_black: bool) -> [u8; 4] {
    let buf: Size<i32, Buffer> = Size::from((4, 4));
    let size: Size<i32, Physical> = Size::from((4, 4));
    let mut texture: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, buf).unwrap();
    let mut fb = renderer.bind(&mut texture).unwrap();
    let mut pass = Pass::default();
    {
        let mut frame = renderer.render(&mut fb, size, Transform::Normal).unwrap();
        frame.clear(Color32F::new(BASE, BASE, BASE, 1.0), &[Rectangle::from_size(size)]).unwrap();
        if let Some(t) = tint {
            frame.with_context(|gl| unsafe { pass.apply(gl, t) }).unwrap();
        }
        // The renderer's normal blending must be intact after the pass.
        if then_draw_half_black {
            frame
                .draw_solid(
                    Rectangle::from_size(size),
                    &[Rectangle::from_size(size)],
                    Color32F::new(0.0, 0.0, 0.0, 0.5),
                )
                .unwrap();
        }
        let _ = frame.finish().unwrap();
    }
    let mapping = renderer.copy_framebuffer(&fb, Rectangle::from_size(buf), Fourcc::Abgr8888).unwrap();
    let px = renderer.map_texture(&mapping).unwrap();
    [px[0], px[1], px[2], px[3]]
}

#[test]
fn the_frame_is_multiplied_by_the_night_light_colour() {
    let Some(mut r) = renderer() else {
        eprintln!("no EGL device: skipping");
        return;
    };
    assert_eq!(pixel(&mut r, None, false)[..3], [153, 153, 153]);
    let tint = color(4000);
    let got = pixel(&mut r, Some(tint), false);
    let want = tint.map(|c| (c * BASE * 255.0).round() as i32);
    for i in 0..3 {
        assert!((got[i] as i32 - want[i]).abs() <= 2, "channel {i}: got {got:?}, want {want:?}");
    }
    assert!(got[2] < got[1] && got[1] < got[0], "warm: {got:?}");

    // Premultiplied blending afterwards: 50% black over the tinted white halves it.
    let after = pixel(&mut r, Some(tint), true);
    for i in 0..3 {
        assert!((after[i] as i32 - got[i] as i32 / 2).abs() <= 2, "channel {i}: {after:?} vs {got:?}");
    }
}
