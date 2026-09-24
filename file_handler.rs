use chardetng::EncodingDetector;
use memchr::memchr;
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

pub const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024 * 1024;
const DETECT_SAMPLE_SIZE: usize = 256 * 1024;

pub struct FileHandler {
    mmap: Mmap,
    line_offsets: Vec<usize>,
    encoding: &'static encoding_rs::Encoding,
    pub total_lines: usize,
    pub file_name: String,
    pub file_size: u64,
    pub encoding_name: &'static str,
}

impl FileHandler {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        if file_size > MAX_FILE_SIZE {
            return Err(format!(
                "檔案大小為 {:.2} GB，超過 4 GB 上限",
                file_size as f64 / 1_073_741_824.0
            )
            .into());
        }

        let mmap = unsafe { Mmap::map(&file)? };
        let encoding = detect_encoding(&mmap);
        let bom_len = bom_length(&mmap, encoding);
        let line_offsets = build_line_index(&mmap, encoding, bom_len);
        let total_lines = line_offsets.len();
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(Self {
            mmap,
            line_offsets,
            encoding,
            total_lines,
            file_name,
            file_size,
            encoding_name: encoding.name(),
        })
    }

    pub fn get_line(&self, line_idx: usize) -> String {
        let Some((start, end)) = self.line_bounds(line_idx) else {
            return String::new();
        };
        let slice = trim_line_ending(&self.mmap[start..end], self.encoding);
        let (cow, _, _) = self.encoding.decode(slice);
        cow.into_owned()
    }

    fn line_bounds(&self, line_idx: usize) -> Option<(usize, usize)> {
        let &start = self.line_offsets.get(line_idx)?;
        let end = self
            .line_offsets
            .get(line_idx + 1)
            .copied()
            .unwrap_or(self.mmap.len());
        Some((start, end))
    }
}

fn detect_encoding(data: &[u8]) -> &'static encoding_rs::Encoding {
    if data.starts_with(b"\xEF\xBB\xBF") {
        return encoding_rs::UTF_8;
    }
    if data.starts_with(b"\xFF\xFE") {
        return encoding_rs::UTF_16LE;
    }
    if data.starts_with(b"\xFE\xFF") {
        return encoding_rs::UTF_16BE;
    }

    let sample = &data[..data.len().min(DETECT_SAMPLE_SIZE)];
    if std::str::from_utf8(sample).is_ok() {
        return encoding_rs::UTF_8;
    }

    let mut detector = EncodingDetector::new();
    detector.feed(sample, true);
    detector.guess(Some(b"hk"), true)
}

fn bom_length(data: &[u8], encoding: &'static encoding_rs::Encoding) -> usize {
    if encoding == encoding_rs::UTF_8 && data.starts_with(b"\xEF\xBB\xBF") {
        3
    } else if (encoding == encoding_rs::UTF_16LE && data.starts_with(b"\xFF\xFE"))
        || (encoding == encoding_rs::UTF_16BE && data.starts_with(b"\xFE\xFF"))
    {
        2
    } else {
        0
    }
}

fn build_line_index(
    data: &[u8],
    encoding: &'static encoding_rs::Encoding,
    start: usize,
) -> Vec<usize> {
    if data.is_empty() || start >= data.len() {
        return Vec::new();
    }

    let mut offsets = vec![start];

    if encoding == encoding_rs::UTF_16LE || encoding == encoding_rs::UTF_16BE {
        let mut pos = start;
        while pos + 1 < data.len() {
            let unit = if encoding == encoding_rs::UTF_16LE {
                u16::from_le_bytes([data[pos], data[pos + 1]])
            } else {
                u16::from_be_bytes([data[pos], data[pos + 1]])
            };
            if unit == 0x000A {
                offsets.push(pos + 2);
            }
            pos += 2;
        }
    } else {
        let mut pos = start;
        while pos < data.len() {
            match memchr(b'\n', &data[pos..]) {
                Some(relative) => {
                    let newline = pos + relative;
                    offsets.push(newline + 1);
                    pos = newline + 1;
                }
                None => break,
            }
        }
    }

    if offsets.last().copied() == Some(data.len()) {
        offsets.pop();
    }
    offsets
}

fn trim_line_ending<'a>(
    mut slice: &'a [u8],
    encoding: &'static encoding_rs::Encoding,
) -> &'a [u8] {
    if encoding == encoding_rs::UTF_16LE {
        if slice.ends_with(&[0x0A, 0x00]) {
            slice = &slice[..slice.len() - 2];
        }
        if slice.ends_with(&[0x0D, 0x00]) {
            slice = &slice[..slice.len() - 2];
        }
    } else if encoding == encoding_rs::UTF_16BE {
        if slice.ends_with(&[0x00, 0x0A]) {
            slice = &slice[..slice.len() - 2];
        }
        if slice.ends_with(&[0x00, 0x0D]) {
            slice = &slice[..slice.len() - 2];
        }
    } else {
        if slice.ends_with(b"\n") {
            slice = &slice[..slice.len() - 1];
        }
        if slice.ends_with(b"\r") {
            slice = &slice[..slice.len() - 1];
        }
    }
    slice
}
