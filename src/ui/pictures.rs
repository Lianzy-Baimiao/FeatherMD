//! Local images for the preview: read and decode off the window thread, keep a
//! bounded cache, draw at display size. Only files on local drives are read; links,
//! junctions, network drives, shares, devices and online-only cloud files are
//! refused before any byte is read, so showing images never reaches the network.
use super::*;
use crate::layout::Picture;
use feathermd::image::{self as rules, Source};
use std::{
    collections::HashMap,
    io::Read,
    path::Path,
    rc::Rc,
    sync::{
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
    },
    thread::JoinHandle,
};
use windows_sys::Win32::{
    Graphics::GdiPlus as gp, Storage::FileSystem::*, System::Com::COINIT_MULTITHREADED,
};

/// GDI+ 32-bit premultiplied BGRA (PixelFormat32bppPARGB).
const PARGB: i32 = 0x000E_200B;
const ORIENTATION: u32 = 0x0112;
/// Decoded copies keep at most this side length; display copies are made from them.
const KEEP_SIDE: u32 = 2048;
/// Decoded pixels kept for all pages, and images one page may show.
const CACHE_BYTES: usize = 160 * 1024 * 1024;
const PAGE_IMAGES: usize = 64;
/// Cloud-file (OneDrive) and deduplicated files are ordinary files behind a
/// reparse point; every other reparse point is a link and is refused.
const TAG_CLOUD: u32 = 0x9000_001A;
const TAG_CLOUD_MASK: u32 = 0x0000_F000;
const TAG_ONEDRIVE: u32 = 0x8000_0021;
const TAG_DEDUP: u32 = 0x8000_0013;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_CDROM: u32 = 5;
const DRIVE_RAMDISK: u32 = 6;
/// True from the decoder's start until its last GDI+ call. GDI+ is shut down only
/// when it is false, so a read that outlives the window cannot use a dead GDI+.
pub(super) static DECODER_ALIVE: AtomicBool = AtomicBool::new(false);

/// Why an image is not shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Missing,
    /// A link, junction, network drive, device or similar path.
    Refused,
    /// An online-only cloud file: reading it would download it.
    Online,
    TooLarge,
    Unreadable,
    /// Past the page's image count; retried when a later page has room.
    Limit,
    /// The page's images already fill the decoded-image budget; not retried.
    Memory,
}
impl Failure {
    pub(super) fn note(self) -> &'static str {
        match self {
            Self::Missing => tr("找不到文件"),
            Self::Refused => tr("不读取链接、网络或设备路径"),
            Self::Online => tr("仅在云端，未下载"),
            Self::TooLarge => tr("文件过大"),
            Self::Unreadable => tr("无法读取"),
            Self::Limit | Self::Memory => tr("已达图片数量上限"),
        }
    }
}

/// Premultiplied BGRA rows, top-down, `width * 4` bytes each, orientation applied.
pub(super) struct Decoded {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixels: Vec<u8>,
}

/// A display-size copy of a decoded image (a 32-bit top-down DIB section).
struct Scaled {
    bitmap: HBITMAP,
    width: i32,
    height: i32,
}
impl Drop for Scaled {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.bitmap);
        }
    }
}

enum State {
    Pending,
    Ready(Arc<Decoded>),
    Failed(Failure),
}
struct Entry {
    state: State,
    scaled: Option<Scaled>,
    /// The last layout pass that referenced this image.
    used: u64,
}

struct Worker {
    requests: Option<Sender<PathBuf>>,
    results: Receiver<(PathBuf, Result<Decoded, Failure>)>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    fn start() -> Self {
        let (requests, inbox) = mpsc::channel::<PathBuf>();
        let (outbox, results) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let quit = stop.clone();
        // Before the thread exists, so shutdown never misses it.
        DECODER_ALIVE.store(true, Ordering::SeqCst);
        let thread = std::thread::spawn(move || unsafe {
            // GDI+ codecs may use COM; this thread never shows UI.
            let com = CoInitializeEx(null(), COINIT_MULTITHREADED as u32);
            while let Ok(path) = inbox.recv() {
                if quit.load(Ordering::SeqCst) {
                    break;
                }
                let result = load(&path);
                if quit.load(Ordering::SeqCst) || outbox.send((path, result)).is_err() {
                    break;
                }
            }
            if com >= 0 {
                CoUninitialize();
            }
            DECODER_ALIVE.store(false, Ordering::SeqCst);
        });
        Self {
            requests: Some(requests),
            results,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // The thread stops after its current file. GDI+ shuts down after the window,
        // so wait briefly; a read that is still stuck leaves GDI+ running.
        self.stop.store(true, Ordering::SeqCst);
        self.requests = None;
        if let Some(thread) = self.thread.take() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !thread.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if thread.is_finished() {
                let _ = thread.join();
            }
        }
    }
}
/// Decoded images of the open document, keyed by their resolved path.
#[derive(Default)]
pub(super) struct Pictures {
    entries: HashMap<Rc<Path>, Entry>,
    /// Current layout pass; entries used by older passes may be evicted.
    pass: u64,
    /// Images requested by the current pass.
    wanted: usize,
    /// Decoded bytes held by Ready entries.
    bytes: usize,
    worker: Option<Worker>,
}
impl Pictures {
    /// Starts a layout pass. Display copies of images the previous page used are
    /// released; decoded images stay until the cache needs room.
    pub(super) fn begin(&mut self) {
        self.pass += 1;
        self.wanted = 0;
        let previous = self.pass - 1;
        for entry in self.entries.values_mut() {
            if entry.used < previous {
                entry.scaled = None;
            }
        }
    }
    /// Notes an image the page shows, queueing it for decoding when new (or when an
    /// earlier page had no room for it).
    pub(super) fn want(&mut self, path: &Path) {
        let retry = match self.entries.get_mut(path) {
            Some(entry) if entry.used == self.pass => return,
            Some(entry) => {
                entry.used = self.pass;
                matches!(entry.state, State::Failed(Failure::Limit))
            }
            None => true,
        };
        self.wanted += 1;
        if !retry {
            return;
        }
        let state = if self.wanted > PAGE_IMAGES {
            State::Failed(Failure::Limit)
        } else {
            let worker = self.worker.get_or_insert_with(Worker::start);
            match &worker.requests {
                Some(requests) if requests.send(path.to_path_buf()).is_ok() => State::Pending,
                _ => State::Failed(Failure::Unreadable),
            }
        };
        self.entries.insert(
            Rc::from(path),
            Entry {
                state,
                scaled: None,
                used: self.pass,
            },
        );
    }
    /// What layout shows for `src` of a document in `folder`.
    pub(super) fn lookup(&self, src: &str, folder: &Path) -> Picture {
        match rules::resolve(src, folder) {
            Source::Remote => Picture::Note(tr("网络图片不下载")),
            Source::Refused => Picture::Note(Failure::Refused.note()),
            Source::Unsupported => Picture::Note(tr("不支持的格式")),
            Source::Local(path) => match self.entries.get_key_value(path.as_path()) {
                Some((key, entry)) => match &entry.state {
                    State::Ready(image) => Picture::Ready {
                        width: image.width,
                        height: image.height,
                        key: key.clone(),
                    },
                    State::Pending => Picture::Note(tr("正在载入")),
                    State::Failed(failure) => Picture::Note(failure.note()),
                },
                None => Picture::Note(tr("正在载入")),
            },
        }
    }
    /// Takes finished decodes; true when one belongs to the page on screen.
    pub(super) fn receive(&mut self) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let finished: Vec<_> = worker.results.try_iter().collect();
        let mut changed = false;
        for (path, result) in finished {
            let used = match self.entries.get(path.as_path()) {
                // A second reply for a path requested again after a clear.
                Some(Entry {
                    state: State::Ready(_),
                    ..
                }) => continue,
                Some(entry) => entry.used,
                // Cleared since it was requested (another document, images off).
                None => continue,
            };
            let state = match result {
                Ok(image) => {
                    let cost = image.pixels.len();
                    if self.make_room(cost) {
                        self.bytes += cost;
                        State::Ready(Arc::new(image))
                    } else {
                        State::Failed(Failure::Memory)
                    }
                }
                Err(failure) => State::Failed(failure),
            };
            if let Some(entry) = self.entries.get_mut(path.as_path()) {
                entry.state = state;
                entry.scaled = None;
            }
            changed |= used == self.pass;
        }
        changed
    }
    /// Frees decoded images of earlier pages, oldest first, until `cost` more fits.
    fn make_room(&mut self, cost: usize) -> bool {
        while self.bytes + cost > CACHE_BYTES {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, e)| e.used < self.pass && matches!(e.state, State::Ready(_)))
                .min_by_key(|(_, e)| e.used)
                .map(|(key, _)| key.clone());
            let Some(key) = oldest else {
                return false;
            };
            if let Some(Entry {
                state: State::Ready(image),
                ..
            }) = self.entries.remove(&key)
            {
                self.bytes -= image.pixels.len();
            }
        }
        true
    }
    /// Forgets every image (another document, F5, or images turned off). Decodes in
    /// flight are dropped when they arrive.
    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
    /// Images of the current page still being read (diagnostics).
    pub(super) fn pending(&self) -> usize {
        self.entries
            .values()
            .filter(|e| e.used == self.pass && matches!(e.state, State::Pending))
            .count()
    }
    /// Decoded bytes held (diagnostics).
    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }
    /// Display copies (one GDI bitmap each) held (diagnostics).
    pub(super) fn bitmaps(&self) -> usize {
        self.entries.values().filter(|e| e.scaled.is_some()).count()
    }
    /// Draws image `key` into `target`, making its display copy when the size
    /// changed; false when there was nothing to draw.
    pub(super) unsafe fn draw(&mut self, dc: HDC, key: &Path, target: RECT) -> bool {
        let (width, height) = (target.right - target.left, target.bottom - target.top);
        let Some(entry) = self.entries.get_mut(key) else {
            return false;
        };
        let State::Ready(image) = &entry.state else {
            return false;
        };
        if entry
            .scaled
            .as_ref()
            .is_none_or(|s| (s.width, s.height) != (width, height))
        {
            entry.scaled = None;
            entry.scaled = scale(dc, image, width, height);
        }
        let Some(scaled) = &entry.scaled else {
            return false;
        };
        let source = CreateCompatibleDC(dc);
        let old = SelectObject(source, scaled.bitmap);
        let drawn = GdiAlphaBlend(
            dc,
            target.left,
            target.top,
            width,
            height,
            source,
            0,
            0,
            width,
            height,
            BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            },
        ) != 0;
        SelectObject(source, old);
        DeleteDC(source);
        drawn
    }
}
/// The folder relative image references of `document` start from, as a
/// drive-absolute path (a relative command-line path is made absolute, and a
/// `\\?\C:\` prefix is dropped). None for documents on shares.
pub(super) fn folder(document: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(document).ok()?;
    let parent = absolute.parent()?.to_string_lossy().into_owned();
    let parent = parent.strip_prefix(r"\\?\").unwrap_or(&parent);
    let b = parent.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':').then(|| PathBuf::from(parent))
}

/// Whether `path` (drive-absolute, normalised by `image::resolve`) may be read
/// without leaving this computer: a local drive, no links on the way, not an
/// online-only cloud file. Only directory entries are queried; nothing is opened.
unsafe fn check(path: &Path) -> Result<(), Failure> {
    let text = path.to_string_lossy();
    let Some(root) = text.get(..3).filter(|r| r.ends_with(":\\")) else {
        return Err(Failure::Refused);
    };
    if !matches!(
        GetDriveTypeW(w(root).as_ptr()),
        DRIVE_FIXED | DRIVE_REMOVABLE | DRIVE_CDROM | DRIVE_RAMDISK
    ) {
        return Err(Failure::Refused);
    }
    // Every folder from the drive root down, then the file itself.
    let mut parts: Vec<&Path> = path.ancestors().filter(|p| p.parent().is_some()).collect();
    parts.reverse();
    for (index, part) in parts.iter().enumerate() {
        let mut data: WIN32_FIND_DATAW = zeroed();
        let find = FindFirstFileW(w(&part.to_string_lossy()).as_ptr(), &mut data);
        if find == INVALID_HANDLE_VALUE {
            return Err(Failure::Missing);
        }
        FindClose(find);
        let attributes = data.dwFileAttributes;
        let last = index + 1 == parts.len();
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            let tag = data.dwReserved0;
            let cloud = (tag & !TAG_CLOUD_MASK) == TAG_CLOUD || tag == TAG_ONEDRIVE;
            if !cloud && tag != TAG_DEDUP {
                return Err(Failure::Refused);
            }
        }
        if last {
            if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                return Err(Failure::Missing);
            }
            if attributes & FILE_ATTRIBUTE_DEVICE != 0 {
                return Err(Failure::Refused);
            }
            if attributes
                & (FILE_ATTRIBUTE_OFFLINE
                    | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
                    | FILE_ATTRIBUTE_RECALL_ON_OPEN)
                != 0
            {
                return Err(Failure::Online);
            }
        } else if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(Failure::Missing);
        }
    }
    Ok(())
}

/// Reads at most `limit` bytes of a checked file; a longer file is refused.
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, Failure> {
    let file = std::fs::File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Failure::Missing,
        _ => Failure::Unreadable,
    })?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::Unreadable)?;
    if bytes.len() as u64 > limit {
        return Err(Failure::TooLarge);
    }
    Ok(bytes)
}

/// Worker: a resolved local path to display-ready pixels.
unsafe fn load(path: &Path) -> Result<Decoded, Failure> {
    check(path)?;
    let bytes = read(path, rules::MAX_BYTES)?;
    decode(&bytes, KEEP_SIDE)
}

/// GDI+ rotate/flip for an EXIF orientation (1 = as stored).
fn rotate_flip(orientation: u16) -> Option<gp::RotateFlipType> {
    Some(match orientation {
        2 => gp::RotateNoneFlipX,
        3 => gp::Rotate180FlipNone,
        4 => gp::Rotate180FlipX,
        5 => gp::Rotate90FlipX,
        6 => gp::Rotate90FlipNone,
        7 => gp::Rotate270FlipX,
        8 => gp::Rotate270FlipNone,
        _ => return None,
    })
}

unsafe fn orientation(image: *mut gp::GpImage) -> u16 {
    let mut size = 0u32;
    if gp::GdipGetPropertyItemSize(image, ORIENTATION, &mut size) != gp::Ok
        || (size as usize) < size_of::<gp::PropertyItem>() + 2
        || size > 4096
    {
        return 1;
    }
    // u64 storage keeps the PropertyItem header aligned.
    let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
    let item = buffer.as_mut_ptr().cast::<gp::PropertyItem>();
    if gp::GdipGetPropertyItem(image, ORIENTATION, size, item) != gp::Ok
        || (*item).value.is_null()
        || (*item).length < 2
    {
        return 1;
    }
    (*item).value.cast::<u16>().read_unaligned()
}

/// Releases a COM object through its IUnknown vtable.
unsafe fn release(object: *mut core::ffi::c_void) {
    if !object.is_null() {
        let vtable = *(object as *const *const usize);
        let release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 =
            std::mem::transmute(*vtable.add(2));
        release(object);
    }
}

/// Decodes encoded image `bytes` (any GDI+ raster type), applies the EXIF
/// orientation and shrinks the result to at most `max_side` pixels per side.
pub(super) unsafe fn decode(bytes: &[u8], max_side: u32) -> Result<Decoded, Failure> {
    if !rules::raster(bytes) {
        return Err(Failure::Unreadable);
    }
    let Ok(length) = u32::try_from(bytes.len()) else {
        return Err(Failure::TooLarge);
    };
    let stream = SHCreateMemStream(bytes.as_ptr(), length);
    if stream.is_null() {
        return Err(Failure::Unreadable);
    }
    let mut bitmap: *mut gp::GpBitmap = null_mut();
    let status = gp::GdipCreateBitmapFromStream(stream, &mut bitmap);
    let result = if status == gp::Ok && !bitmap.is_null() {
        let image = bitmap.cast::<gp::GpImage>();
        let result = convert(image, max_side);
        gp::GdipDisposeImage(image);
        result
    } else {
        Err(Failure::Unreadable)
    };
    release(stream);
    result
}

unsafe fn convert(image: *mut gp::GpImage, max_side: u32) -> Result<Decoded, Failure> {
    let (mut width, mut height) = (0u32, 0u32);
    gp::GdipGetImageWidth(image, &mut width);
    gp::GdipGetImageHeight(image, &mut height);
    // Decompression bombs: a small file may claim an enormous canvas.
    if width == 0
        || height == 0
        || width > rules::MAX_SIDE
        || height > rules::MAX_SIDE
        || u64::from(width) * u64::from(height) > rules::MAX_PIXELS
    {
        return Err(if width == 0 || height == 0 {
            Failure::Unreadable
        } else {
            Failure::TooLarge
        });
    }
    if let Some(turn) = rotate_flip(orientation(image)) {
        gp::GdipImageRotateFlip(image, turn);
        gp::GdipGetImageWidth(image, &mut width);
        gp::GdipGetImageHeight(image, &mut height);
    }
    let fit = (max_side as f32 / width.max(height) as f32).min(1.0);
    let w = ((width as f32 * fit).round() as u32).max(1);
    let h = ((height as f32 * fit).round() as u32).max(1);
    let mut pixels = vec![0u8; w as usize * h as usize * 4];
    if !draw_into(image, (width, height), &mut pixels, w, h) {
        return Err(Failure::Unreadable);
    }
    Ok(Decoded {
        width: w,
        height: h,
        pixels,
    })
}

/// Draws all of `image` (`size` pixels) into premultiplied BGRA `pixels` of
/// `width` × `height`, filtering when it shrinks.
unsafe fn draw_into(
    image: *mut gp::GpImage,
    size: (u32, u32),
    pixels: &mut [u8],
    width: u32,
    height: u32,
) -> bool {
    let mut target: *mut gp::GpBitmap = null_mut();
    if gp::GdipCreateBitmapFromScan0(
        width as i32,
        height as i32,
        width as i32 * 4,
        PARGB,
        pixels.as_mut_ptr(),
        &mut target,
    ) != gp::Ok
    {
        return false;
    }
    let mut graphics: *mut gp::GpGraphics = null_mut();
    let mut attributes: *mut gp::GpImageAttributes = null_mut();
    let drawn = gp::GdipGetImageGraphicsContext(target.cast(), &mut graphics) == gp::Ok && {
        gp::GdipSetCompositingMode(graphics, gp::CompositingModeSourceCopy);
        gp::GdipSetInterpolationMode(graphics, gp::InterpolationModeHighQualityBicubic);
        gp::GdipSetPixelOffsetMode(graphics, gp::PixelOffsetModeHighQuality);
        // Mirrored edges keep the filter from fading the border.
        if gp::GdipCreateImageAttributes(&mut attributes) == gp::Ok {
            gp::GdipSetImageAttributesWrapMode(attributes, gp::WrapModeTileFlipXY, 0, 0);
        }
        gp::GdipDrawImageRectRectI(
            graphics,
            image,
            0,
            0,
            width as i32,
            height as i32,
            0,
            0,
            size.0 as i32,
            size.1 as i32,
            gp::UnitPixel,
            attributes,
            0,
            null_mut(),
        ) == gp::Ok
    };
    if !attributes.is_null() {
        gp::GdipDisposeImageAttributes(attributes);
    }
    if !graphics.is_null() {
        gp::GdipDeleteGraphics(graphics);
    }
    gp::GdipDisposeImage(target.cast());
    drawn
}

/// A `width` × `height` display copy of `image` as a premultiplied DIB section.
unsafe fn scale(dc: HDC, image: &Decoded, width: i32, height: i32) -> Option<Scaled> {
    if width <= 0 || height <= 0 || width > 16_384 || height > 16_384 {
        return None;
    }
    let mut info: BITMAPINFO = zeroed();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut core::ffi::c_void = null_mut();
    let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    if bitmap.is_null() || bits.is_null() {
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        return None;
    }
    let scaled = Scaled {
        bitmap,
        width,
        height,
    };
    let pixels =
        std::slice::from_raw_parts_mut(bits.cast::<u8>(), width as usize * height as usize * 4);
    let mut source: *mut gp::GpBitmap = null_mut();
    if gp::GdipCreateBitmapFromScan0(
        image.width as i32,
        image.height as i32,
        image.width as i32 * 4,
        PARGB,
        image.pixels.as_ptr().cast_mut(),
        &mut source,
    ) != gp::Ok
    {
        return None;
    }
    let drawn = draw_into(
        source.cast(),
        (image.width, image.height),
        pixels,
        width as u32,
        height as u32,
    );
    gp::GdipDisposeImage(source.cast());
    GdiFlush();
    drawn.then_some(scaled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::core::GUID;

    // GDI+ built-in encoders.
    const PNG: GUID = GUID {
        data1: 0x557c_f406,
        data2: 0x1a04,
        data3: 0x11d3,
        data4: [0x9a, 0x73, 0x00, 0x00, 0xf8, 0x1e, 0xf3, 0x2e],
    };
    const JPEG: GUID = GUID {
        data1: 0x557c_f401,
        data2: 0x1a04,
        data3: 0x11d3,
        data4: [0x9a, 0x73, 0x00, 0x00, 0xf8, 0x1e, 0xf3, 0x2e],
    };

    /// A file in a scratch folder of its own for each test (tests run in parallel).
    fn scratch(test: &str, name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("feathermd-pictures-{}-{test}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    /// Encodes premultiplied BGRA `pixels` with a GDI+ encoder and returns the file.
    unsafe fn encode(pixels: &[u8], width: i32, height: i32, encoder: &GUID) -> Vec<u8> {
        let mut bitmap: *mut gp::GpBitmap = null_mut();
        assert_eq!(
            gp::GdipCreateBitmapFromScan0(
                width,
                height,
                width * 4,
                PARGB,
                pixels.as_ptr(),
                &mut bitmap
            ),
            gp::Ok
        );
        let name = if encoder.data1 == PNG.data1 {
            "encoded.png"
        } else {
            "encoded.jpg"
        };
        let path = scratch("decode", name);
        let status = gp::GdipSaveImageToFile(
            bitmap.cast(),
            w(&path.to_string_lossy()).as_ptr(),
            encoder,
            null(),
        );
        gp::GdipDisposeImage(bitmap.cast());
        assert_eq!(status, gp::Ok);
        std::fs::read(&path).unwrap()
    }

    /// `jpeg` with an EXIF block saying "rotate 90° clockwise to view" (orientation 6).
    fn with_orientation_6(jpeg: &[u8]) -> Vec<u8> {
        let mut app1 = vec![0xff, 0xe1, 0x00, 0x22];
        app1.extend_from_slice(b"Exif\0\0II*\0\x08\0\0\0");
        // One IFD0 entry: tag 0x0112, SHORT, count 1, value 6; no next IFD.
        app1.extend_from_slice(&[1, 0, 0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(app1.len(), 0x22 + 2);
        let mut out = jpeg[..2].to_vec();
        out.extend_from_slice(&app1);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    fn pixel(image: &Decoded, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * image.width + x) * 4) as usize;
        image.pixels[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn decodes_premultiplied_applies_exif_orientation_and_refuses_bad_input() {
        unsafe {
            let _gdi = smooth::Session::start().unwrap();
            // 4x2 PNG: opaque red, half-transparent blue, transparent; premultiplied BGRA in.
            let mut source = Vec::new();
            for _ in 0..2 {
                source.extend_from_slice(&[0, 0, 255, 255, 128, 0, 0, 128]);
                source.extend_from_slice(&[0, 0, 0, 0, 0, 0, 255, 255]);
            }
            let png = encode(&source, 4, 2, &PNG);
            assert!(rules::raster(&png));
            let image = decode(&png, KEEP_SIDE).unwrap();
            assert_eq!((image.width, image.height), (4, 2));
            assert_eq!(pixel(&image, 0, 0), [0, 0, 255, 255]);
            let half = pixel(&image, 1, 0);
            assert!(
                half[3].abs_diff(128) <= 1 && half[0].abs_diff(128) <= 2,
                "{half:?}"
            );
            assert_eq!(pixel(&image, 2, 1)[3], 0);

            // 40x20 JPEG, green left half, white right half, EXIF orientation 6:
            // shown 20x40 with the green half on top.
            let mut stored = Vec::new();
            for _ in 0..20 {
                for x in 0..40 {
                    stored.extend_from_slice(if x < 20 {
                        &[40, 160, 20, 255]
                    } else {
                        &[255, 255, 255, 255]
                    });
                }
            }
            let jpeg = with_orientation_6(&encode(&stored, 40, 20, &JPEG));
            let image = decode(&jpeg, KEEP_SIDE).unwrap();
            assert_eq!((image.width, image.height), (20, 40));
            let [b, g, r, _] = pixel(&image, 10, 5);
            assert!(
                g > 120 && r < 90 && b < 90,
                "top should be green: {r},{g},{b}"
            );
            let [b, g, r, _] = pixel(&image, 10, 35);
            assert!(
                r > 220 && g > 220 && b > 220,
                "bottom should be white: {r},{g},{b}"
            );

            // Large images shrink to the kept side; giant canvases are refused.
            let wide = vec![255u8; 3000 * 4 * 6];
            let image = decode(&encode(&wide, 3000, 6, &PNG), KEEP_SIDE).unwrap();
            assert_eq!((image.width, image.height), (2048, 4));
            let line = vec![255u8; 16_385 * 4];
            assert_eq!(
                decode(&encode(&line, 16_385, 1, &PNG), KEEP_SIDE).err(),
                Some(Failure::TooLarge)
            );
            // Not raster data, whatever the file is called: never handed to a decoder.
            for bytes in [
                &b"this is not a png"[..],
                b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
                b"\xd7\xcd\xc6\x9a\0\0placeable metafile",
                b"\x01\0\0\0 enhanced metafile",
                b"",
            ] {
                assert_eq!(decode(bytes, KEEP_SIDE).err(), Some(Failure::Unreadable));
            }
            // A PNG signature on a truncated file fails cleanly.
            assert_eq!(
                decode(&png[..20], KEEP_SIDE).err(),
                Some(Failure::Unreadable)
            );
            let _ = std::fs::remove_dir_all(scratch("decode", "x").parent().unwrap());
        }
    }

    #[test]
    fn reads_only_plain_local_files_within_the_limit() {
        unsafe {
            let file = scratch("paths", "plain.png");
            std::fs::write(&file, vec![7u8; 1000]).unwrap();
            assert_eq!(check(&file), Ok(()));
            assert_eq!(read(&file, 1000).map(|b| b.len()), Ok(1000));
            assert_eq!(read(&file, 999).err(), Some(Failure::TooLarge));
            assert_eq!(
                check(&scratch("paths", "absent.png")),
                Err(Failure::Missing)
            );
            assert_eq!(check(file.parent().unwrap()), Err(Failure::Missing));
            // A junction could point anywhere, including a share: refused, as is
            // everything behind it.
            let target = scratch("paths", "target");
            std::fs::create_dir_all(&target).unwrap();
            std::fs::write(target.join("a.png"), [1u8]).unwrap();
            let link = scratch("paths", "link");
            let _ = std::fs::remove_dir(&link);
            let made = std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(&link)
                .arg(&target)
                .output()
                .is_ok_and(|o| o.status.success());
            if made {
                assert_eq!(check(&link.join("a.png")), Err(Failure::Refused));
                assert_eq!(check(&target.join("a.png")), Ok(()));
                std::fs::remove_dir(&link).unwrap();
            }
            // Relative command-line paths and \\?\ prefixes still give a drive folder.
            assert_eq!(
                folder(Path::new(r"\\?\C:\docs\a.md")),
                Some(PathBuf::from(r"C:\docs"))
            );
            assert_eq!(folder(Path::new(r"\\server\share\a.md")), None);
            assert!(folder(Path::new("relative.md")).is_some_and(|f| f.is_absolute()));
            let _ = std::fs::remove_dir_all(file.parent().unwrap());
        }
    }
}
