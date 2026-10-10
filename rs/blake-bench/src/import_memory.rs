//! Current virtual-mapping compatibility with Metal's no-copy import.
//! SDK 26.4.1: mach/vm_region.h, mach/mach_vm.h and unistd.h.

use aruminium::GpuError;
use std::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};
use unimem::Block;

#[repr(C, packed(4))]
#[derive(Default)]
struct RegionInfo {
    protection: c_int,
    max_protection: c_int,
    inheritance: u32,
    offset: u64,
    user_tag: u32,
    ref_count: u32,
    shadow_depth: u16,
    external_pager: u8,
    share_mode: u8,
    is_submap: c_int,
    behavior: c_int,
    object_id: u32,
    user_wired_count: u16,
    flags: u16,
}

const _: () = {
    assert!(size_of::<RegionInfo>() == 48);
    assert!(align_of::<RegionInfo>() == 4);
    assert!(offset_of!(RegionInfo, is_submap) == 32);
};

unsafe extern "C" {
    fn getpagesize() -> c_int;
    static mach_task_self_: u32;
    fn mach_vm_region_recurse(
        task: u32,
        address: *mut u64,
        size: *mut u64,
        depth: *mut u32,
        info: *mut c_int,
        count: *mut u32,
    ) -> c_int;
}

pub(crate) fn error(message: impl Into<String>) -> GpuError {
    GpuError::BufferCreationFailed(message.into())
}

pub(crate) fn page_size() -> Result<usize, GpuError> {
    // SAFETY: No pointer or initialization preconditions.
    let page = unsafe { getpagesize() };
    if page <= 0 {
        return Err(error("runtime page size is not positive"));
    }
    Ok(page as usize)
}

pub(crate) fn capacity(actual: usize, maximum: usize) -> Result<(), GpuError> {
    if actual > maximum {
        return Err(error("batch backing exceeds device maximum buffer length"));
    }
    Ok(())
}

fn extent(pointer: usize, actual: usize, page: usize) -> Result<u64, GpuError> {
    if page == 0
        || pointer == 0
        || actual == 0
        || !pointer.is_multiple_of(8)
        || !pointer.is_multiple_of(page)
        || !actual.is_multiple_of(page)
    {
        return Err(error(
            "batch address must be ulong-aligned and address/extent runtime-page aligned",
        ));
    }
    let end = pointer
        .checked_add(actual)
        .ok_or_else(|| error("batch address end overflow"))?;
    u64::try_from(end).map_err(|_| error("batch address exceeds native address domain"))
}

fn contained(start: u64, size: u64, pointer: u64, end: u64) -> Result<(), GpuError> {
    let region_end = start
        .checked_add(size)
        .ok_or_else(|| error("VM region end overflow"))?;
    if start > pointer || end > region_end {
        return Err(error("batch backing crosses the current VM region"));
    }
    Ok(())
}

// The caller keeps this private Block alive, with no exported alias or mapping
// mutation, through import and completion. This checks the current VM mapping;
// it does not establish physical contiguity or a universal IOSurface guarantee.
pub(crate) fn validate(block: &Block) -> Result<(), GpuError> {
    let pointer = block.address() as usize;
    let end = extent(pointer, block.size(), page_size()?)?;
    let pointer =
        u64::try_from(pointer).map_err(|_| error("batch address exceeds native address domain"))?;
    let mut start = pointer;
    let mut size = 0;
    let mut depth = u32::MAX;
    let mut info = RegionInfo::default();
    let mut count = 12;
    // SAFETY: Initialized, writable ABI-sized outputs for this task's live
    // mapping. Maximum depth reaches a leaf. No object-name send right is returned.
    let status = unsafe {
        mach_vm_region_recurse(
            mach_task_self_,
            &mut start,
            &mut size,
            &mut depth,
            (&mut info as *mut RegionInfo).cast(),
            &mut count,
        )
    };
    if status != 0 || count != 12 {
        return Err(error(format!(
            "VM region query failed: status {status}, count {count}"
        )));
    }
    if info.is_submap != 0 {
        return Err(error("VM region query did not return a leaf"));
    }
    if info.protection & 3 != 3 {
        return Err(error("batch VM region is not readable and writable"));
    }
    contained(start, size, pointer, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_errors_are_distinct_without_fabricated_native_owners() {
        assert!(extent(16, 16, 16).is_ok());
        for args in [
            (0, 16, 16),
            (4, 4, 4),
            (17, 16, 16),
            (16, 17, 16),
            (16, 16, 0),
            (16, 0, 16),
            (usize::MAX, 1, 1),
        ] {
            assert!(extent(args.0, args.1, args.2).is_err());
        }
        assert!(contained(16, 32, 16, 48).is_ok());
        assert!(contained(17, 32, 16, 48).is_err());
        assert!(contained(16, 31, 16, 48).is_err());
        assert!(contained(u64::MAX, 1, u64::MAX, u64::MAX).is_err());
        assert!(capacity(17, 16)
            .unwrap_err()
            .to_string()
            .contains("device maximum"));
    }
}
