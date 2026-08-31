//! Raw ObjC FFI to create an MTLTextureDescriptor for the R-table.
//!
//! aruminium exposes `Gpu::texture(desc: ObjcId)` and
//! `Texture::replace_region`, but not a descriptor builder. We construct
//! the descriptor via direct `objc_msgSend`.

use std::ffi::{c_char, c_void};

use aruminium::ffi::{MTLOrigin, MTLRegion, MTLSize};
use aruminium::{Gpu, GpuError, Texture};

type ObjcId = *mut c_void;
type ObjcSel = *mut c_void;

#[link(name = "objc", kind = "dylib")]
extern "C" {
    fn objc_msgSend();
    fn objc_getClass(name: *const c_char) -> ObjcId;
    fn sel_registerName(name: *const c_char) -> ObjcSel;
}

// MTLPixelFormatRGBA32Uint
const PIXEL_FORMAT_RGBA32_UINT: u64 = 123;
// MTLTextureUsageShaderRead
const TEXTURE_USAGE_SHADER_READ: u64 = 1;
// MTLStorageModeShared
const STORAGE_MODE_SHARED: u64 = 0;

fn cls(name: &str) -> ObjcId {
    let cname = std::ffi::CString::new(name).unwrap();
    unsafe { objc_getClass(cname.as_ptr()) }
}

fn sel(name: &str) -> ObjcSel {
    let cname = std::ffi::CString::new(name).unwrap();
    unsafe { sel_registerName(cname.as_ptr()) }
}

pub fn create_2d_rgba32u_texture(
    gpu: &Gpu,
    width: usize,
    height: usize,
) -> Result<Texture, GpuError> {
    let descriptor_class = cls("MTLTextureDescriptor");
    if descriptor_class.is_null() {
        return Err(GpuError::TextureCreationFailed(
            "MTLTextureDescriptor class not found".into(),
        ));
    }
    let make_2d = sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:");
    let set_usage = sel("setUsage:");
    let set_storage = sel("setStorageMode:");

    type MakeDescFn = unsafe extern "C" fn(ObjcId, ObjcSel, u64, u64, u64, u8) -> ObjcId;
    let make_desc: MakeDescFn = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    let desc = unsafe {
        make_desc(
            descriptor_class,
            make_2d,
            PIXEL_FORMAT_RGBA32_UINT,
            width as u64,
            height as u64,
            0,
        )
    };
    if desc.is_null() {
        return Err(GpuError::TextureCreationFailed(
            "texture2DDescriptor returned nil".into(),
        ));
    }

    type SetU64Fn = unsafe extern "C" fn(ObjcId, ObjcSel, u64);
    let setu64: SetU64Fn = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { setu64(desc, set_usage, TEXTURE_USAGE_SHADER_READ) };
    unsafe { setu64(desc, set_storage, STORAGE_MODE_SHARED) };

    unsafe { gpu.texture(desc) }
}

/// Upload a contiguous byte buffer into the texture's raw bytes.
/// The texture must have been created with the exact width/height that
/// covers `src_len` at 16 bytes per texel.
pub fn upload_rtable(
    texture: &Texture,
    src_bytes: *const u8,
    src_len: usize,
    width: usize,
    height: usize,
) {
    let region = MTLRegion {
        origin: MTLOrigin { x: 0, y: 0, z: 0 },
        size: MTLSize {
            width,
            height,
            depth: 1,
        },
    };
    let bytes_per_row = width * 16; // RGBA32Uint = 16 bytes/texel
    assert_eq!(
        bytes_per_row * height,
        src_len,
        "texture size != src buffer size"
    );
    unsafe {
        texture.replace_region(region, 0, src_bytes as *const c_void, bytes_per_row);
    }
}
