//! HEIC/HEIF (and AVIF) support via the system `libheif`, loaded at runtime.
//!
//! The library is resolved with `dlopen`-style loading on first use instead of
//! a build-time C dependency: the default build stays pure Rust, and system /
//! side-by-side copies of libheif are picked up automatically. When the library
//! (or a decoder plugin) is missing, HEIF decoding returns a friendly error and
//! all other formats are unaffected.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;
use std::sync::OnceLock;

use libloading::Library;

use crate::error::{Error, Result};
use crate::image_buf::ImageBuf;

// ---------------------------------------------------------------------------
// libheif C ABI (subset, see libheif/heif.h)
// ---------------------------------------------------------------------------

#[repr(C)]
struct HeifError {
    code: c_int,
    subcode: c_int,
    message: *const c_char,
}

#[repr(C)]
struct HeifContext {
    _p: [u8; 0],
}
#[repr(C)]
struct HeifImageHandle {
    _p: [u8; 0],
}
#[repr(C)]
struct HeifImage {
    _p: [u8; 0],
}

const HEIF_COLORSPACE_RGB: c_int = 1; // heif_colorspace_RGB
const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11; // heif_chroma_interleaved_RGBA
const HEIF_CHANNEL_INTERLEAVED: c_int = 10; // heif_channel_interleaved

type FnContextAlloc = unsafe extern "C" fn() -> *mut HeifContext;
type FnContextFree = unsafe extern "C" fn(*mut HeifContext);
type FnReadFromFile = unsafe extern "C" fn(*mut HeifContext, *const c_char, *const c_void) -> HeifError;
type FnGetPrimaryHandle = unsafe extern "C" fn(*mut HeifContext, *mut *mut HeifImageHandle) -> HeifError;
type FnHandleGetSize = unsafe extern "C" fn(*const HeifImageHandle) -> c_int;
type FnHandleRelease = unsafe extern "C" fn(*mut HeifImageHandle);
type FnDecodeImage =
    unsafe extern "C" fn(*const HeifImageHandle, *mut *mut HeifImage, c_int, c_int, *const c_void) -> HeifError;
type FnGetPlaneReadonly = unsafe extern "C" fn(*const HeifImage, c_int, *mut c_int) -> *const u8;
type FnImageRelease = unsafe extern "C" fn(*mut HeifImage);
type FnGetMetaCount = unsafe extern "C" fn(*const HeifImageHandle, *const c_char) -> c_int;
type FnGetMetaIds = unsafe extern "C" fn(*const HeifImageHandle, *const c_char, *mut u32, c_int) -> c_int;
type FnGetMetaSize = unsafe extern "C" fn(*const HeifImageHandle, u32) -> usize;
type FnGetMetadata = unsafe extern "C" fn(*const HeifImageHandle, u32, *mut c_void) -> HeifError;
type FnGetVersion = unsafe extern "C" fn() -> *const c_char;
// 内嵌缩略图 API(iPhone 的 HEIC 通常带 320–512px 缩略图,解码比主图快一个数量级)
type FnGetThumbCount = unsafe extern "C" fn(*const HeifImageHandle) -> c_int;
type FnGetThumbIds = unsafe extern "C" fn(*const HeifImageHandle, *mut u32, c_int) -> c_int;
type FnGetThumbnail =
    unsafe extern "C" fn(*const HeifImageHandle, u32, *mut *mut HeifImageHandle) -> HeifError;
type FnSetMaxThreads = unsafe extern "C" fn(*mut HeifContext, c_int);

struct Fns {
    context_alloc: FnContextAlloc,
    context_free: FnContextFree,
    read_from_file: FnReadFromFile,
    get_primary_image_handle: FnGetPrimaryHandle,
    handle_get_width: FnHandleGetSize,
    handle_get_height: FnHandleGetSize,
    handle_release: FnHandleRelease,
    decode_image: FnDecodeImage,
    image_get_plane_readonly: FnGetPlaneReadonly,
    image_release: FnImageRelease,
    get_meta_count: Option<FnGetMetaCount>,
    get_meta_ids: Option<FnGetMetaIds>,
    get_meta_size: Option<FnGetMetaSize>,
    get_metadata: Option<FnGetMetadata>,
    get_version: Option<FnGetVersion>,
    thumb_count: Option<FnGetThumbCount>,
    thumb_ids: Option<FnGetThumbIds>,
    get_thumbnail: Option<FnGetThumbnail>,
    set_max_threads: Option<FnSetMaxThreads>,
}

struct Heif {
    _lib: Library,
    fns: Fns,
}

static HEIF: OnceLock<std::result::Result<Heif, String>> = OnceLock::new();

const LIB_CANDIDATES: &[&str] = &[
    // macOS .app bundle(CI 打包产物;@executable_path 由 dyld 展开)
    "@executable_path/../Frameworks/libheif.1.dylib",
    "@executable_path/../Frameworks/libheif.dylib",
    // 系统库
    "libheif.so.1",
    "libheif.so",
    "libheif.1.dylib",
    "libheif.dylib",
    "libheif.dll",
];

/// 可执行文件旁的库文件(便携分发时随包附带)。
fn exe_dir_candidates() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["libheif.so.1", "libheif.1.dylib", "libheif.dll"] {
                out.push(dir.join(name).to_string_lossy().into_owned());
            }
        }
    }
    out
}

fn instance() -> Result<&'static Heif> {
    match HEIF.get_or_init(|| unsafe { open() }) {
        Ok(heif) => Ok(heif),
        Err(msg) => Err(Error::HeifUnavailable(msg.clone())),
    }
}

/// Whether a usable libheif library was found on this system.
pub fn is_available() -> bool {
    instance().is_ok()
}

unsafe fn open() -> std::result::Result<Heif, String> {
    let mut last_error = String::from("libheif not found");
    let candidates: Vec<String> = exe_dir_candidates()
        .into_iter()
        .chain(LIB_CANDIDATES.iter().map(|s| s.to_string()))
        .collect();
    for candidate in &candidates {
        let lib = match unsafe { Library::new(candidate.as_str()) } {
            Ok(lib) => lib,
            Err(e) => {
                last_error = format!("{candidate}: {e}");
                continue;
            }
        };
        let fns = (|| -> std::result::Result<Fns, libloading::Error> {
            unsafe {
                Ok(Fns {
                    context_alloc: *lib.get(b"heif_context_alloc\0")?,
                    context_free: *lib.get(b"heif_context_free\0")?,
                    read_from_file: *lib.get(b"heif_context_read_from_file\0")?,
                    get_primary_image_handle: *lib.get(b"heif_context_get_primary_image_handle\0")?,
                    handle_get_width: *lib.get(b"heif_image_handle_get_width\0")?,
                    handle_get_height: *lib.get(b"heif_image_handle_get_height\0")?,
                    handle_release: *lib.get(b"heif_image_handle_release\0")?,
                    decode_image: *lib.get(b"heif_decode_image\0")?,
                    image_get_plane_readonly: *lib.get(b"heif_image_get_plane_readonly\0")?,
                    image_release: *lib.get(b"heif_image_release\0")?,
                    get_meta_count: lib.get(b"heif_image_handle_get_number_of_metadata_blocks\0").ok().map(|s| *s),
                    get_meta_ids: lib.get(b"heif_image_handle_get_list_of_metadata_block_IDs\0").ok().map(|s| *s),
                    get_meta_size: lib.get(b"heif_image_handle_get_metadata_size\0").ok().map(|s| *s),
                    get_metadata: lib.get(b"heif_image_handle_get_metadata\0").ok().map(|s| *s),
                    get_version: lib.get(b"heif_get_version\0").ok().map(|s| *s),
                    thumb_count: lib
                        .get(b"heif_image_handle_get_number_of_thumbnails\0")
                        .ok()
                        .map(|s| *s),
                    thumb_ids: lib
                        .get(b"heif_image_handle_get_list_of_thumbnail_IDs\0")
                        .ok()
                        .map(|s| *s),
                    get_thumbnail: lib
                        .get(b"heif_image_handle_get_thumbnail\0")
                        .ok()
                        .map(|s| *s),
                    set_max_threads: lib
                        .get(b"heif_context_set_max_decoding_threads\0")
                        .ok()
                        .map(|s| *s),
                })
            }
        })();
        match fns {
            Ok(fns) => {
                if let Some(get_version) = fns.get_version {
                    unsafe {
                        let ptr = get_version();
                        if !ptr.is_null() {
                            log::info!("libheif {} loaded from {candidate}", CStr::from_ptr(ptr).to_string_lossy());
                        }
                    }
                }
                return Ok(Heif { _lib: lib, fns });
            }
            Err(e) => last_error = format!("{candidate}: {e}"),
        }
    }
    Err(last_error)
}

fn check(err: HeifError, what: &str) -> Result<()> {
    if err.code == 0 {
        return Ok(());
    }
    let message = if err.message.is_null() {
        "unknown error".to_owned()
    } else {
        unsafe { CStr::from_ptr(err.message) }.to_string_lossy().into_owned()
    };
    Err(Error::Decode(format!(
        "heif {what} failed (code {} / subcode {}): {message}",
        err.code, err.subcode
    )))
}

/// Runs a cleanup function on drop so every early return releases libheif
/// resources.
struct Guard<F: FnMut()>(F);

impl<F: FnMut()> Drop for Guard<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// Open `path` and run `f` on its primary image handle.
fn with_primary_handle<T>(
    path: &Path,
    f: impl FnOnce(&Fns, *const HeifImageHandle) -> Result<T>,
) -> Result<T> {
    with_primary_handle_threads(path, None, f)
}

/// Same as [`with_primary_handle`], but optionally caps libheif's decoding
/// threads (thumbnails use 1 to avoid oversubscribing the worker pool).
fn with_primary_handle_threads<T>(
    path: &Path,
    max_threads: Option<c_int>,
    f: impl FnOnce(&Fns, *const HeifImageHandle) -> Result<T>,
) -> Result<T> {
    let heif = instance()?;
    let fns = &heif.fns;
    let c_path = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| Error::Decode("path contains a NUL byte".into()))?;

    unsafe {
        let ctx = (fns.context_alloc)();
        if ctx.is_null() {
            return Err(Error::Decode("heif: context allocation failed".into()));
        }
        let _ctx_guard = Guard(|| (fns.context_free)(ctx));

        if let (Some(threads), Some(set)) = (max_threads, fns.set_max_threads) {
            set(ctx, threads);
        }

        check((fns.read_from_file)(ctx, c_path.as_ptr(), std::ptr::null()), "read_from_file")?;

        let mut handle: *mut HeifImageHandle = std::ptr::null_mut();
        check((fns.get_primary_image_handle)(ctx, &mut handle), "get_primary_image_handle")?;
        if handle.is_null() {
            return Err(Error::Decode("heif: file has no primary image".into()));
        }
        let _handle_guard = Guard(|| (fns.handle_release)(handle));

        f(fns, handle)
    }
}

/// Decode the primary image of a HEIF/HEIC/AVIF file to RGBA.
pub fn decode(path: &Path) -> Result<ImageBuf> {
    with_primary_handle(path, |fns, handle| decode_handle(fns, handle))
}

/// Decode one image handle (primary image or embedded thumbnail) to RGBA.
fn decode_handle(fns: &Fns, handle: *const HeifImageHandle) -> Result<ImageBuf> {
    unsafe {
        let width = (fns.handle_get_width)(handle);
        let height = (fns.handle_get_height)(handle);
        if width <= 0 || height <= 0 {
            return Err(Error::Decode(format!("heif: invalid dimensions {width}x{height}")));
        }

        let mut image: *mut HeifImage = std::ptr::null_mut();
        check(
            (fns.decode_image)(
                handle,
                &mut image,
                HEIF_COLORSPACE_RGB,
                HEIF_CHROMA_INTERLEAVED_RGBA,
                std::ptr::null(),
            ),
            "decode_image",
        )?;
        if image.is_null() {
            return Err(Error::Decode("heif: decode returned a null image".into()));
        }
        let _image_guard = Guard(|| (fns.image_release)(image));

        let mut stride: c_int = 0;
        let plane = (fns.image_get_plane_readonly)(image, HEIF_CHANNEL_INTERLEAVED, &mut stride);
        if plane.is_null() || stride <= 0 {
            return Err(Error::Decode("heif: no interleaved RGBA plane".into()));
        }

        let (w, h) = (width as usize, height as usize);
        let stride = stride as usize;
        let row_bytes = w * 4;
        if stride < row_bytes {
            return Err(Error::Decode(format!("heif: stride {stride} < row bytes {row_bytes}")));
        }
        let mut rgba = vec![0u8; row_bytes * h];
        for y in 0..h {
            let src = plane.add(y * stride);
            std::ptr::copy_nonoverlapping(src, rgba.as_mut_ptr().add(y * row_bytes), row_bytes);
        }
        Ok(ImageBuf::new(w as u32, h as u32, rgba))
    }
}

/// 解码内嵌缩略图(若文件包含)。iPhone 的 HEIC 通常带 320–512px 缩略图,
/// 解码速度比主图快一个数量级,用于网格缩略图。限制解码线程为 1,避免
/// 与缩略图工作线程池互相抢占。
pub fn decode_thumbnail(path: &Path) -> Result<ImageBuf> {
    with_primary_handle_threads(path, Some(1), |fns, handle| unsafe {
        let (Some(count_fn), Some(ids_fn), Some(thumb_fn)) =
            (fns.thumb_count, fns.thumb_ids, fns.get_thumbnail)
        else {
            return Err(Error::Decode("heif: thumbnail API unavailable".into()));
        };
        let count = count_fn(handle);
        if count <= 0 {
            return Err(Error::Decode("heif: no embedded thumbnail".into()));
        }
        let mut ids = vec![0u32; count as usize];
        let found = ids_fn(handle, ids.as_mut_ptr(), count);
        if found <= 0 {
            return Err(Error::Decode("heif: no thumbnail IDs".into()));
        }
        let mut thumb: *mut HeifImageHandle = std::ptr::null_mut();
        check(thumb_fn(handle, ids[0], &mut thumb), "get_thumbnail")?;
        if thumb.is_null() {
            return Err(Error::Decode("heif: null thumbnail handle".into()));
        }
        let _thumb_guard = Guard(|| (fns.handle_release)(thumb));
        decode_handle(fns, thumb)
    })
}

/// Width/height of the primary image without decoding pixels.
pub fn dimensions(path: &Path) -> Result<(u32, u32)> {
    with_primary_handle(path, |fns, handle| unsafe {
        let w = (fns.handle_get_width)(handle);
        let h = (fns.handle_get_height)(handle);
        if w <= 0 || h <= 0 {
            Err(Error::Decode(format!("heif: invalid dimensions {w}x{h}")))
        } else {
            Ok((w as u32, h as u32))
        }
    })
}

/// Raw EXIF blob stored in the HEIF metadata item (as-is from the container;
/// callers must handle the possible 4-byte TIFF-offset prefix).
pub fn exif_bytes(path: &Path) -> Result<Vec<u8>> {
    with_primary_handle(path, |fns, handle| unsafe {
        let (Some(get_count), Some(get_ids), Some(get_size), Some(get_metadata)) =
            (fns.get_meta_count, fns.get_meta_ids, fns.get_meta_size, fns.get_metadata)
        else {
            return Err(Error::Decode("heif: metadata API unavailable".into()));
        };
        let filter = b"Exif\0".as_ptr() as *const c_char;

        let count = get_count(handle, filter);
        if count <= 0 {
            return Err(Error::Decode("heif: no EXIF metadata".into()));
        }
        let mut ids = vec![0u32; count as usize];
        let found = get_ids(handle, filter, ids.as_mut_ptr(), count);
        if found <= 0 {
            return Err(Error::Decode("heif: no EXIF item IDs".into()));
        }
        let id = ids[0];

        let size = get_size(handle, id);
        if size == 0 {
            return Err(Error::Decode("heif: empty EXIF metadata".into()));
        }
        let mut buf = vec![0u8; size];
        check(get_metadata(handle, id, buf.as_mut_ptr() as *mut c_void), "get_metadata")?;
        Ok(buf)
    })
}
