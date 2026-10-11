//! Private SDK-compatible descriptor and encoder calls for the fixed V9 profile.
use crate::BenchError;
use aruminium::ffi::{MTLOrigin, MTLRegion, MTLSize};
use aruminium::{Encoder, Gpu, GpuError, Texture};
use std::ffi::{c_char, c_void, CStr};
type Id = *mut c_void;

#[link(name = "objc", kind = "dylib")]
extern "C" {
    fn objc_msgSend();
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Id;
}
const RGBA32_UINT: usize = 123;
const WIDTH: usize = 16384;
const ROW_BYTES: usize = WIDTH * 16;

fn selector(name: &CStr) -> Id {
    // SAFETY: static NUL-terminated SDK selector; registration retains its name.
    unsafe { sel_registerName(name.as_ptr()) }
}

fn geometry(rows: u64, bytes: usize) -> Result<usize, BenchError> {
    if rows == 0 || !rows.is_multiple_of(8192) || rows > 1 << 27 {
        return Err(BenchError::InvalidInput(
            "V9 requires 8192-row multiples up to 2^27",
        ));
    }
    let height =
        usize::try_from(rows / 8192).map_err(|_| BenchError::InvalidInput("texture height"))?;
    if height.checked_mul(ROW_BYTES) != Some(bytes) {
        return Err(BenchError::InvalidInput("texture logical byte length"));
    }
    Ok(height)
}

pub(super) fn create(gpu: &Gpu, rows: u64, bytes: &[u8]) -> Result<Texture, BenchError> {
    let height = geometry(rows, bytes.len())?;
    aruminium::autorelease_pool(|| {
        // SAFETY: exact SDK ObjC class, selectors and LP64 NSUInteger/bool ABI.
        // Descriptor is autoreleased inside this pool; gpu.texture returns a
        // separately retained native Texture before the pool drains.
        let texture = unsafe {
            let class = objc_getClass(c"MTLTextureDescriptor".as_ptr());
            if class.is_null() {
                return Err(GpuError::TextureCreationFailed(
                    "MTLTextureDescriptor class missing".into(),
                )
                .into());
            }
            let make: unsafe extern "C" fn(Id, Id, usize, usize, usize, bool) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            let desc = make(
                class,
                selector(c"texture2DDescriptorWithPixelFormat:width:height:mipmapped:"),
                RGBA32_UINT,
                WIDTH,
                height,
                false,
            );
            if desc.is_null() {
                return Err(GpuError::TextureCreationFailed(
                    "texture descriptor returned nil".into(),
                )
                .into());
            }
            let set: unsafe extern "C" fn(Id, Id, usize) =
                std::mem::transmute(objc_msgSend as *const c_void);
            set(desc, selector(c"setUsage:"), 1); // MTLTextureUsageShaderRead
            set(desc, selector(c"setStorageMode:"), 0); // MTLStorageModeShared
            gpu.texture(desc)?
        };
        if texture.width() != WIDTH
            || texture.height() != height
            || texture.depth() != 1
            || texture.pixel_format() != RGBA32_UINT
        {
            return Err(GpuError::TextureCreationFailed(
                "texture dimensions/format differ from V9 profile".into(),
            )
            .into());
        }
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width: WIDTH,
                height,
                depth: 1,
            },
        };
        // SAFETY: fresh private texture has no GPU use. Exact initialized logical
        // bytes cover the entire sole mip; immediate CPU copy finishes before
        // the slice borrow ends. Apple requests zero stride for a single row.
        unsafe {
            texture.replace_region(
                region,
                0,
                bytes.as_ptr().cast(),
                if height == 1 { 0 } else { ROW_BYTES },
            );
        }
        Ok(texture)
    })
}

pub(super) fn bind(encoder: &Encoder, texture: &Texture) {
    // SAFETY: live retained encoder/texture, exact SDK NSUInteger argument.
    // Owner keeps texture initialized and retained until command completion.
    unsafe {
        let set: unsafe extern "C" fn(Id, Id, Id, usize) =
            std::mem::transmute(objc_msgSend as *const c_void);
        set(
            encoder.as_raw(),
            selector(c"setTexture:atIndex:"),
            texture.as_raw(),
            0,
        );
    }
}

#[cfg(test)]
mod tests;
