//! Night light: warms the screen by multiplying every pixel by the colour of
//! a lower colour temperature.
//!
//! Nested mode does it with one GL pass after each frame (blend: dst * colour),
//! which only runs while Night light is on. The DRM backend will load the same
//! colour into the display's gamma table instead, which is free.
//! Screenshots and window previews render separately, so they stay untinted.

use smithay::backend::renderer::gles::ffi;

/// Neutral white: no change.
pub const NEUTRAL: u32 = 6500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NightLight {
    pub on: bool,
    /// Kelvin, clamped to 1900-6500 when applied.
    pub temperature: u32,
}

impl NightLight {
    /// The multiplier for each channel, or None when nothing should change.
    pub fn multiplier(&self) -> Option<[f32; 3]> {
        (self.on && self.temperature < NEUTRAL).then(|| color(self.temperature))
    }
}

/// RGB of a black body at `kelvin`, scaled so 6500 K is white. Uses Tanner
/// Helland's fit of the CIE data (the same curve redshift and gammastep
/// approximate), which is accurate to a few percent from 1000 to 40000 K.
pub fn color(kelvin: u32) -> [f32; 3] {
    fn raw(kelvin: f64) -> [f64; 3] {
        let t = kelvin / 100.0;
        let r = if t <= 66.0 { 255.0 } else { 329.698727446 * (t - 60.0).powf(-0.1332047592) };
        let g = if t <= 66.0 {
            99.4708025861 * t.ln() - 161.1195681661
        } else {
            288.1221695283 * (t - 60.0).powf(-0.0755148492)
        };
        let b = if t >= 66.0 {
            255.0
        } else if t <= 19.0 {
            0.0
        } else {
            138.5177312231 * (t - 10.0).ln() - 305.0447927307
        };
        [r, g, b].map(|c| c.clamp(0.0, 255.0))
    }
    let k = kelvin.clamp(1900, NEUTRAL) as f64;
    let (c, white) = (raw(k), raw(NEUTRAL as f64));
    [0, 1, 2].map(|i| (c[i] / white[i]).clamp(0.0, 1.0) as f32)
}

/// The compiled multiply pass (made once, on first use).
#[derive(Default)]
pub struct Pass {
    program: Option<(ffi::types::GLuint, ffi::types::GLint, ffi::types::GLint)>,
}

const VERTEX: &str = "attribute vec2 pos; void main() { gl_Position = vec4(pos, 0.0, 1.0); }\0";
const FRAGMENT: &str = "precision mediump float; uniform vec3 tint; void main() { gl_FragColor = vec4(tint, 1.0); }\0";

impl Pass {
    /// Multiplies what's in the bound framebuffer by `tint`. Leaves the GL
    /// state as Smithay's renderer set it (premultiplied-alpha blending).
    ///
    /// # Safety
    /// Must run inside a frame's `with_context`, with its framebuffer bound.
    pub unsafe fn apply(&mut self, gl: &ffi::Gles2, tint: [f32; 3]) {
        let Some((program, pos, tint_loc)) = self.program.or_else(|| unsafe { compile(gl) }) else { return };
        self.program = Some((program, pos, tint_loc));
        const QUAD: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
        unsafe {
            gl.UseProgram(program);
            gl.Uniform3f(tint_loc, tint[0], tint[1], tint[2]);
            gl.BindBuffer(ffi::ARRAY_BUFFER, 0);
            gl.EnableVertexAttribArray(pos as u32);
            gl.VertexAttribPointer(pos as u32, 2, ffi::FLOAT, ffi::FALSE, 0, QUAD.as_ptr().cast());
            gl.Enable(ffi::BLEND);
            gl.BlendFunc(ffi::ZERO, ffi::SRC_COLOR); // result = framebuffer * tint
            gl.DrawArrays(ffi::TRIANGLE_STRIP, 0, 4);
            gl.BlendFunc(ffi::ONE, ffi::ONE_MINUS_SRC_ALPHA);
            gl.DisableVertexAttribArray(pos as u32);
            gl.UseProgram(0);
        }
    }
}

unsafe fn compile(gl: &ffi::Gles2) -> Option<(ffi::types::GLuint, ffi::types::GLint, ffi::types::GLint)> {
    unsafe fn shader(gl: &ffi::Gles2, kind: ffi::types::GLenum, src: &str) -> Option<ffi::types::GLuint> {
        unsafe {
            let s = gl.CreateShader(kind);
            let ptr = src.as_ptr().cast();
            gl.ShaderSource(s, 1, &ptr, std::ptr::null());
            gl.CompileShader(s);
            let mut ok = 0;
            gl.GetShaderiv(s, ffi::COMPILE_STATUS, &mut ok);
            (ok != 0).then_some(s)
        }
    }
    unsafe {
        let vs = shader(gl, ffi::VERTEX_SHADER, VERTEX)?;
        let fs = shader(gl, ffi::FRAGMENT_SHADER, FRAGMENT)?;
        let program = gl.CreateProgram();
        gl.AttachShader(program, vs);
        gl.AttachShader(program, fs);
        gl.LinkProgram(program);
        gl.DeleteShader(vs);
        gl.DeleteShader(fs);
        let mut ok = 0;
        gl.GetProgramiv(program, ffi::LINK_STATUS, &mut ok);
        if ok == 0 {
            tracing::warn!("night light shader failed to link");
            return None;
        }
        let pos = gl.GetAttribLocation(program, c"pos".as_ptr());
        let tint = gl.GetUniformLocation(program, c"tint".as_ptr());
        Some((program, pos, tint))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_white_and_lower_is_warmer() {
        let white = color(6500);
        assert!(white.iter().all(|c| (c - 1.0).abs() < 1e-6), "{white:?}");
        let warm = color(4000);
        assert_eq!(warm[0], 1.0, "red is never reduced");
        assert!(warm[1] < 1.0 && warm[2] < warm[1], "less green, even less blue: {warm:?}");
        let warmer = color(3000);
        assert!(warmer[1] < warm[1] && warmer[2] < warm[2], "lower kelvin is warmer");
        assert_eq!(color(500), color(1900), "clamped at 1900 K");
        assert_eq!(color(9000), color(6500), "never cooler than neutral");
    }

    #[test]
    fn only_applies_when_on_and_warm() {
        assert_eq!(NightLight { on: false, temperature: 4000 }.multiplier(), None);
        assert_eq!(NightLight { on: true, temperature: 6500 }.multiplier(), None);
        assert!(NightLight { on: true, temperature: 4000 }.multiplier().is_some());
    }
}
