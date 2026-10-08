//! Safe wrappers over the VA-API objects FilmCraft uses: the display (a DRM render node), a
//! decode session (config + context + surfaces), parameter and slice-data buffers, and reading a
//! decoded surface back into a planar [`VideoFrame`]. Every libva call's status is checked;
//! failures are `Err(String)` with libva's own message. Objects are released in `Drop`.

use std::ffi::{CStr, c_int, c_void};
use std::os::fd::AsRawFd;

use filmcraft_frame::VideoFrame;

use super::ffi::*;
use crate::biplanar::{Biplanar, Geometry};

/// Render nodes tried, in order (`FILMCRAFT_VAAPI_DEVICE` names one explicitly).
fn candidate_nodes() -> Vec<String> {
    if let Some(n) = std::env::var_os("FILMCRAFT_VAAPI_DEVICE") {
        return vec![n.to_string_lossy().into_owned()];
    }
    (128..136).map(|i| format!("/dev/dri/renderD{i}")).filter(|p| std::path::Path::new(p).exists()).collect()
}

/// An initialised VA display on one render node.
pub struct Display {
    api: Api,
    dpy: VADisplay,
    _fd: std::fs::File,
    pub node: String,
    pub version: (i32, i32),
}

// SAFETY: a VADisplay is a handle into libva, which serialises access to a display internally
// (libva's public API is documented as thread-safe per display); FilmCraft never shares one
// `Display` between threads at the same time anyway: it is owned by one decoder, and `Send`
// only lets that decoder move to another thread.
unsafe impl Send for Display {}

impl Display {
    /// Open the first render node whose driver decodes `profile` (VLD) at `rt_format`.
    pub fn open_for(profile: VAProfile, rt_format: u32) -> Result<Display, String> {
        let mut why = Vec::new();
        for node in candidate_nodes() {
            match Display::open(&node) {
                Ok(d) => match d.supports(profile, rt_format) {
                    Ok(true) => return Ok(d),
                    Ok(false) => why.push(format!("{node}: profile {profile} not supported")),
                    Err(e) => why.push(format!("{node}: {e}")),
                },
                Err(e) => why.push(format!("{node}: {e}")),
            }
        }
        Err(if why.is_empty() { "no DRM render node".into() } else { why.join("; ") })
    }

    pub fn open(node: &str) -> Result<Display, String> {
        let api = Api::load()?;
        let fd = std::fs::OpenOptions::new().read(true).write(true).open(node).map_err(|e| format!("{node}: {e}"))?;
        // SAFETY: `fd` is an open render node that outlives the display (kept in `_fd`).
        let dpy = unsafe { (api.get_display_drm)(fd.as_raw_fd()) };
        if dpy.is_null() {
            return Err("vaGetDisplayDRM returned no display".into());
        }
        let (mut major, mut minor) = (0, 0);
        // SAFETY: `dpy` is a valid display; the out-pointers are live locals.
        let st = unsafe { (api.initialize)(dpy, &mut major, &mut minor) };
        let d = Display { api, dpy, _fd: fd, node: node.to_string(), version: (major, minor) };
        d.check(st, "vaInitialize")?;
        Ok(d)
    }

    fn check(&self, st: VAStatus, what: &str) -> Result<(), String> {
        if st == VA_STATUS_SUCCESS {
            return Ok(());
        }
        // SAFETY: vaErrorStr returns a pointer to a static, NUL-terminated string for any status.
        let msg = unsafe { (self.api.error_str)(st) };
        let text = if msg.is_null() {
            String::new()
        } else {
            // SAFETY: non-null, NUL-terminated, static (see above).
            unsafe { CStr::from_ptr(msg) }.to_string_lossy().into_owned()
        };
        Err(format!("{what}: {text} ({st})"))
    }

    pub fn profiles(&self) -> Result<Vec<VAProfile>, String> {
        // SAFETY: valid display.
        let max = unsafe { (self.api.max_num_profiles)(self.dpy) }.clamp(0, 256);
        let mut v = vec![0; max as usize];
        let mut n: c_int = 0;
        // SAFETY: `v` has room for `max` entries, as libva requires.
        let st = unsafe { (self.api.query_config_profiles)(self.dpy, v.as_mut_ptr(), &mut n) };
        self.check(st, "vaQueryConfigProfiles")?;
        v.truncate(n.clamp(0, max) as usize);
        Ok(v)
    }

    pub fn entrypoints(&self, profile: VAProfile) -> Result<Vec<VAEntrypoint>, String> {
        // SAFETY: valid display.
        let max = unsafe { (self.api.max_num_entrypoints)(self.dpy) }.clamp(0, 64);
        let mut v = vec![0; max as usize];
        let mut n: c_int = 0;
        // SAFETY: `v` has room for `max` entries.
        let st = unsafe { (self.api.query_config_entrypoints)(self.dpy, profile, v.as_mut_ptr(), &mut n) };
        self.check(st, "vaQueryConfigEntrypoints")?;
        v.truncate(n.clamp(0, max) as usize);
        Ok(v)
    }

    /// Config attributes of a profile's VLD entry point (`VA_ATTRIB_NOT_SUPPORTED` when absent).
    pub fn attributes(&self, profile: VAProfile, kinds: &[VAConfigAttribType]) -> Result<Vec<u32>, String> {
        let mut a: Vec<VAConfigAttrib> = kinds.iter().map(|k| VAConfigAttrib { type_: *k, value: 0 }).collect();
        // SAFETY: `a` holds `a.len()` initialised attributes for libva to fill in.
        let st = unsafe { (self.api.get_config_attributes)(self.dpy, profile, VAEntrypointVLD, a.as_mut_ptr(), a.len() as c_int) };
        self.check(st, "vaGetConfigAttributes")?;
        Ok(a.iter().map(|x| x.value).collect())
    }

    /// Whether `profile` decodes (VLD) at `rt_format`.
    pub fn supports(&self, profile: VAProfile, rt_format: u32) -> Result<bool, String> {
        if !self.profiles()?.contains(&profile) || !self.entrypoints(profile)?.contains(&VAEntrypointVLD) {
            return Ok(false);
        }
        let rt = self.attributes(profile, &[VAConfigAttribRTFormat])?;
        Ok(rt.first().is_some_and(|v| *v != VA_ATTRIB_NOT_SUPPORTED && v & rt_format != 0))
    }

    /// Largest picture the decoder takes, if the driver says.
    pub fn max_size(&self, profile: VAProfile) -> Result<Option<(u32, u32)>, String> {
        let a = self.attributes(profile, &[VAConfigAttribMaxPictureWidth, VAConfigAttribMaxPictureHeight])?;
        Ok(match (a.first(), a.get(1)) {
            (Some(w), Some(h)) if *w != VA_ATTRIB_NOT_SUPPORTED && *h != VA_ATTRIB_NOT_SUPPORTED => Some((*w, *h)),
            _ => None,
        })
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        // SAFETY: the display was initialised and every object made on it is released first
        // (sessions own their display and drop their objects before it).
        unsafe { (self.api.terminate)(self.dpy) };
    }
}

/// Parameter structures libva may copy from memory.
///
/// # Safety
/// Implementors are `#[repr(C)]` structures of integers and arrays of integers, with no pointers,
/// laid out exactly as libva's C declaration (checked in `abi_tests.rs`). They may contain
/// padding: they are only handed to libva by pointer (C copies the bytes), never viewed as a Rust
/// byte slice.
pub unsafe trait VaParam: Copy {}
// SAFETY: repr(C) integer-only structures from `ffi.rs`, layouts checked in abi_tests.rs.
unsafe impl VaParam for VAPictureParameterBufferH264 {}
// SAFETY: as above.
unsafe impl VaParam for VAIQMatrixBufferH264 {}
// SAFETY: as above.
unsafe impl VaParam for VASliceParameterBufferH264 {}
// SAFETY: as above.
unsafe impl VaParam for VAPictureParameterBufferHEVC {}
// SAFETY: as above.
unsafe impl VaParam for VASliceParameterBufferHEVC {}
// SAFETY: as above.
unsafe impl VaParam for VAIQMatrixBufferHEVC {}

/// A decode session: config, context and its pool of surfaces.
pub struct Session {
    disp: Display,
    config: VAConfigID,
    context: VAContextID,
    surfaces: Vec<VASurfaceID>,
    pub width: u32,
    pub height: u32,
    pub bits: u32,
}

impl Session {
    /// A VLD session for `profile`, `width`×`height` coded size, `count` surfaces of 8- or 10-bit
    /// 4:2:0.
    pub fn new(disp: Display, profile: VAProfile, bits: u32, width: u32, height: u32, count: usize) -> Result<Session, String> {
        if width == 0 || height == 0 || width > 16384 || height > 16384 || count == 0 || count > 64 {
            return Err(format!("unsupported session {width}x{height} with {count} surfaces"));
        }
        let rt = if bits > 8 { VA_RT_FORMAT_YUV420_10 } else { VA_RT_FORMAT_YUV420 };
        let mut attr = [VAConfigAttrib { type_: VAConfigAttribRTFormat, value: rt }];
        let mut config = VA_INVALID_ID;
        // SAFETY: valid display; `attr` and `config` are live.
        let st = unsafe { (disp.api.create_config)(disp.dpy, profile, VAEntrypointVLD, attr.as_mut_ptr(), 1, &mut config) };
        disp.check(st, "vaCreateConfig")?;
        let mut s = Session { disp, config, context: VA_INVALID_ID, surfaces: Vec::new(), width, height, bits };
        let mut surfaces = vec![VA_INVALID_SURFACE; count];
        // SAFETY: `surfaces` has `count` slots; no surface attributes are passed.
        let st = unsafe { (s.disp.api.create_surfaces)(s.disp.dpy, rt, width, height, surfaces.as_mut_ptr(), count as u32, std::ptr::null_mut(), 0) };
        s.disp.check(st, "vaCreateSurfaces")?;
        s.surfaces = surfaces;
        let mut ctx = VA_INVALID_ID;
        // SAFETY: valid config and surfaces; `ctx` is live.
        let st = unsafe {
            (s.disp.api.create_context)(
                s.disp.dpy,
                s.config,
                width as c_int,
                height as c_int,
                VA_PROGRESSIVE,
                s.surfaces.as_mut_ptr(),
                s.surfaces.len() as c_int,
                &mut ctx,
            )
        };
        s.disp.check(st, "vaCreateContext")?;
        s.context = ctx;
        Ok(s)
    }

    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }

    /// The VA surface id of slot `i` (for reference lists).
    pub fn surface_id(&self, i: usize) -> Option<VASurfaceID> {
        self.surfaces.get(i).copied()
    }

    /// Create a buffer from `count` elements of `elem` bytes at `data`.
    ///
    /// # Safety
    /// `data` must point to `elem * count` bytes that stay valid for the duration of the call.
    unsafe fn buffer_raw(&self, ty: VABufferType, data: *const c_void, elem: usize, count: usize) -> Result<Buffer<'_>, String> {
        if elem == 0 || count == 0 || elem > u32::MAX as usize || count > u32::MAX as usize {
            return Err("buffer size out of range".into());
        }
        let mut id = VA_INVALID_ID;
        // SAFETY: the caller guarantees `data` covers `elem * count` bytes; libva copies them into
        // its own buffer before returning and never writes through the pointer.
        let st = unsafe { (self.disp.api.create_buffer)(self.disp.dpy, self.context, ty, elem as u32, count as u32, data as *mut c_void, &mut id) };
        self.disp.check(st, "vaCreateBuffer")?;
        Ok(Buffer { s: self, id })
    }

    /// A parameter buffer holding `items` (handed to libva by pointer: padding bytes are copied by
    /// C, never read as Rust bytes).
    pub fn params<T: VaParam>(&self, ty: VABufferType, items: &[T]) -> Result<Buffer<'_>, String> {
        // SAFETY: `items` is a live slice of `items.len()` values of `size_of::<T>()` bytes each.
        unsafe { self.buffer_raw(ty, items.as_ptr().cast::<c_void>(), std::mem::size_of::<T>(), items.len()) }
    }

    /// A slice-data buffer.
    pub fn data(&self, bytes: &[u8]) -> Result<Buffer<'_>, String> {
        // SAFETY: `bytes` is a live slice of `bytes.len()` bytes.
        unsafe { self.buffer_raw(VASliceDataBufferType, bytes.as_ptr().cast::<c_void>(), bytes.len(), 1) }
    }

    /// Decode one picture into surface slot `target` from `buffers` (picture parameters first).
    pub fn decode(&self, target: usize, buffers: &[Buffer<'_>]) -> Result<(), String> {
        let surface = self.surface_id(target).ok_or("no such surface")?;
        let mut ids: Vec<VABufferID> = buffers.iter().map(|b| b.id).collect();
        // SAFETY: valid context and surface.
        let st = unsafe { (self.disp.api.begin_picture)(self.disp.dpy, self.context, surface) };
        self.disp.check(st, "vaBeginPicture")?;
        // SAFETY: `ids` are buffers created on this context and still alive (borrowed above).
        let st = unsafe { (self.disp.api.render_picture)(self.disp.dpy, self.context, ids.as_mut_ptr(), ids.len() as c_int) };
        let rendered = self.disp.check(st, "vaRenderPicture");
        // SAFETY: a picture was begun on this context; it must be ended even after a render error.
        let st = unsafe { (self.disp.api.end_picture)(self.disp.dpy, self.context) };
        rendered?;
        self.disp.check(st, "vaEndPicture")
    }

    /// Wait for surface slot `slot` and copy its picture (`crop` within the coded size) out.
    pub fn read(&self, slot: usize, g: &Geometry) -> Result<VideoFrame, String> {
        let surface = self.surface_id(slot).ok_or("no such surface")?;
        // SAFETY: valid surface.
        let st = unsafe { (self.disp.api.sync_surface)(self.disp.dpy, surface) };
        self.disp.check(st, "vaSyncSurface")?;
        let (fourcc, bpp) = if self.bits > 8 { (VA_FOURCC_P010, 24) } else { (VA_FOURCC_NV12, 12) };
        let mut fmt = VAImageFormat { fourcc, byte_order: 1, bits_per_pixel: bpp, ..Default::default() };
        let mut img = VAImage::default();
        // SAFETY: `fmt` and `img` are live; libva fills `img`.
        let st = unsafe { (self.disp.api.create_image)(self.disp.dpy, &mut fmt, self.width as c_int, self.height as c_int, &mut img) };
        self.disp.check(st, "vaCreateImage")?;
        let image = Image { s: self, img };
        // SAFETY: valid surface and image of the surface's size.
        let st = unsafe { (self.disp.api.get_image)(self.disp.dpy, surface, 0, 0, self.width, self.height, image.img.image_id) };
        self.disp.check(st, "vaGetImage")?;
        let mut ptr: *mut c_void = std::ptr::null_mut();
        // SAFETY: `image.img.buf` is the image's data buffer.
        let st = unsafe { (self.disp.api.map_buffer)(self.disp.dpy, image.img.buf, &mut ptr) };
        self.disp.check(st, "vaMapBuffer")?;
        let size = image.img.data_size as usize;
        let out = if ptr.is_null() {
            Err("vaMapBuffer returned no data".to_string())
        } else {
            // SAFETY: libva maps `data_size` bytes at `ptr` until vaUnmapBuffer below; the slice
            // does not outlive this block.
            let data = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), size) };
            let (o0, o1) = (image.img.offsets[0] as usize, image.img.offsets[1] as usize);
            let stride = image.img.pitches[0] as usize;
            match (data.get(o0..), data.get(o1..)) {
                (Some(luma), Some(chroma)) if image.img.pitches[1] as usize == stride && o1 > o0 => {
                    let rows = (o1 - o0) / stride.max(1);
                    crate::biplanar::to_frame(&Biplanar { luma, chroma, stride, rows }, g)
                }
                _ => Err(format!("unexpected image layout {:?}/{:?}", image.img.offsets, image.img.pitches)),
            }
        };
        // SAFETY: mapped above.
        let st = unsafe { (self.disp.api.unmap_buffer)(self.disp.dpy, image.img.buf) };
        let frame = out?;
        self.disp.check(st, "vaUnmapBuffer")?;
        Ok(frame)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: each id was created on this display (VA_INVALID_ID ones are skipped) and is
        // released once, children before parents.
        unsafe {
            if self.context != VA_INVALID_ID {
                (self.disp.api.destroy_context)(self.disp.dpy, self.context);
            }
            if !self.surfaces.is_empty() {
                (self.disp.api.destroy_surfaces)(self.disp.dpy, self.surfaces.as_mut_ptr(), self.surfaces.len() as c_int);
            }
            if self.config != VA_INVALID_ID {
                (self.disp.api.destroy_config)(self.disp.dpy, self.config);
            }
        }
    }
}

/// A libva buffer, destroyed when dropped.
pub struct Buffer<'a> {
    s: &'a Session,
    id: VABufferID,
}

impl Drop for Buffer<'_> {
    fn drop(&mut self) {
        // SAFETY: created on this session's display, destroyed once.
        unsafe { (self.s.disp.api.destroy_buffer)(self.s.disp.dpy, self.id) };
    }
}

struct Image<'a> {
    s: &'a Session,
    img: VAImage,
}

impl Drop for Image<'_> {
    fn drop(&mut self) {
        // SAFETY: created by vaCreateImage on this display, destroyed once.
        unsafe { (self.s.disp.api.destroy_image)(self.s.disp.dpy, self.img.image_id) };
    }
}
