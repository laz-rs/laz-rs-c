#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use crate::LazrsResult::{LAZRS_OK, LAZRS_OTHER};
use laz;
use laz::{LasZipError, LazItem};
use libc;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

#[derive(Copy, Clone)]
struct CFile {
    fh: NonNull<libc::FILE>,
}

unsafe impl Send for CFile {}

impl Read for CFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        unsafe {
            let buff_ptr = buf.as_mut_ptr();
            let n_read = libc::fread(
                buff_ptr as *mut libc::c_void,
                std::mem::size_of::<u8>(),
                buf.len(),
                self.fh.as_ptr(),
            );
            if n_read < buf.len() {
                let err_code = libc::ferror(self.fh.as_ptr());
                if err_code != 0 {
                    return Err(std::io::Error::from_raw_os_error(err_code));
                }
            }
            Ok(n_read)
        }
    }
}

impl Seek for CFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        unsafe {
            let (pos, whence) = match pos {
                SeekFrom::Start(pos) => {
                    assert!(pos < std::i32::MAX as u64);
                    (pos as i32, libc::SEEK_SET)
                }
                SeekFrom::End(pos) => {
                    assert!(pos < i64::from(std::i32::MAX));
                    (pos as i32, libc::SEEK_END)
                }
                SeekFrom::Current(pos) => {
                    assert!(pos < i64::from(std::i32::MAX));
                    (pos as i32, libc::SEEK_CUR)
                }
            };

            if pos != 0 && whence != libc::SEEK_CUR {
                let result = libc::fseek(self.fh.as_ptr(), pos.into(), whence);
                if result != 0 {
                    return Err(std::io::Error::from_raw_os_error(result));
                }
            }
            let position = libc::ftell(self.fh.as_ptr());
            Ok(position as u64)
        }
    }
}

impl Write for CFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        unsafe {
            let n_written = libc::fwrite(
                buf.as_ptr() as *const libc::c_void,
                std::mem::size_of::<u8>(),
                buf.len(),
                self.fh.as_ptr(),
            );
            if n_written < buf.len() {
                let err_code = libc::ferror(self.fh.as_ptr());
                if err_code != 0 {
                    return Err(std::io::Error::from_raw_os_error(err_code));
                }
            }
            Ok(n_written)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unsafe {
            let status = libc::fflush(self.fh.as_ptr());
            if status == libc::EOF {
                return Err(std::io::Error::from_raw_os_error(libc::ferror(
                    self.fh.as_ptr(),
                )));
            }
        }
        Ok(())
    }
}

enum CSource<'a> {
    Memory(Cursor<&'a [u8]>),
    File(CFile),
}

impl<'a> Read for CSource<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            CSource::Memory(cursor) => cursor.read(buf),
            CSource::File(file) => file.read(buf),
        }
    }
}

impl<'a> Seek for CSource<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            CSource::Memory(cursor) => cursor.seek(pos),
            CSource::File(file) => file.seek(pos),
        }
    }
}

#[repr(C)]
pub enum LazrsResult {
    LAZRS_OK,
    LAZRS_UNKNOWN_LAZ_ITEM,
    LAZRS_UNKNOWN_LAZ_ITEM_VERSION,
    LAZRS_UNKNOWN_COMPRESSOR_TYPE,
    LAZRS_UNSUPPORTED_COMPRESSOR_TYPE,
    LAZRS_UNSUPPORTED_POINT_FORMAT,
    LAZRS_IO_ERROR,
    LAZRS_MISSING_CHUNK_TABLE,
    LAZRS_OTHER,
}

impl From<laz::LasZipError> for LazrsResult {
    fn from(e: LasZipError) -> Self {
        match e {
            LasZipError::UnknownLazItem(_) => LazrsResult::LAZRS_UNKNOWN_LAZ_ITEM,
            LasZipError::UnsupportedLazItemVersion(_, _) => {
                LazrsResult::LAZRS_UNKNOWN_LAZ_ITEM_VERSION
            }
            LasZipError::UnknownCompressorType(_) => LazrsResult::LAZRS_UNKNOWN_COMPRESSOR_TYPE,
            LasZipError::UnsupportedCompressorType(_) => {
                LazrsResult::LAZRS_UNSUPPORTED_COMPRESSOR_TYPE
            }
            LasZipError::UnsupportedPointFormat(_) => LazrsResult::LAZRS_UNSUPPORTED_POINT_FORMAT,
            LasZipError::IoError(_) => LazrsResult::LAZRS_IO_ERROR,
            LasZipError::MissingChunkTable => LazrsResult::LAZRS_MISSING_CHUNK_TABLE,
            _ => LazrsResult::LAZRS_OTHER,
        }
    }
}

impl From<Result<(), laz::LasZipError>> for LazrsResult {
    fn from(r: Result<(), LasZipError>) -> Self {
        match r {
            Ok(_) => LazrsResult::LAZRS_OK,
            Err(e) => e.into(),
        }
    }
}

impl From<std::io::Result<()>> for LazrsResult {
    fn from(r: std::io::Result<()>) -> Self {
        match r {
            Ok(_) => LazrsResult::LAZRS_OK,
            Err(_) => LazrsResult::LAZRS_IO_ERROR,
        }
    }
}

pub struct LasZipDecompressor {
    decompressor: laz::LasZipDecompressor<'static, CSource<'static>>,
}

/// Creates a new decompressor that decompresses data from the given file
///
/// If an error occurs, the returned result will be something other that LAZRS_OK
/// and the decompressor will be set to NULL.
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompressor_new_file(
    decompressor: *mut *mut LasZipDecompressor,
    fh: *mut libc::FILE,
    laszip_vlr_record_data: *const u8,
    record_data_len: u16,
) -> LazrsResult {
    if decompressor.is_null() || fh.is_null() {
        return LazrsResult::LAZRS_OTHER;
    }

    let vlr_data = std::slice::from_raw_parts(laszip_vlr_record_data, usize::from(record_data_len));
    let vlr = match laz::LazVlr::from_buffer(vlr_data) {
        Ok(vlr) => vlr,
        Err(error) => {
            return error.into();
        }
    };

    let cfile = CFile {
        fh: NonNull::new_unchecked(fh),
    };

    match laz::LasZipDecompressor::new(CSource::File(cfile), vlr) {
        Ok(d) => {
            *decompressor = Box::into_raw(Box::new(LasZipDecompressor { decompressor: d }));
            LazrsResult::LAZRS_OK
        }
        Err(error) => {
            *decompressor = std::ptr::null_mut::<LasZipDecompressor>();
            LazrsResult::LAZRS_OTHER
        }
    }
}

/// Creates a new decompressor that decompresses data from the
/// given buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompressor_new_buffer(
    data: *const u8,
    size: usize,
    laszip_vlr_record_data: *const u8,
    record_data_len: u16,
) -> *mut LasZipDecompressor {
    let vlr_data = std::slice::from_raw_parts(laszip_vlr_record_data, usize::from(record_data_len));
    let vlr = match laz::LazVlr::from_buffer(vlr_data) {
        Ok(vlr) => vlr,
        Err(error) => {
            eprintln!("{}", error);
            return core::ptr::null_mut::<LasZipDecompressor>();
        }
    };

    let source = CSource::Memory {
        0: Cursor::new(std::slice::from_raw_parts(data, size)),
    };
    let dbox = Box::new(LasZipDecompressor {
        decompressor: laz::LasZipDecompressor::new(source, vlr).unwrap(),
    });
    Box::into_raw(dbox)
}

/// Deletes the decompressor
///
/// decompressor can be NULL
#[no_mangle]
pub unsafe extern "C" fn lazrs_delete_decompressor(decompressor: *mut LasZipDecompressor) {
    if !decompressor.is_null() {
        Box::from_raw(decompressor);
    }
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_decompress_one(
    decompressor: *mut LasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> LazrsResult {
    debug_assert!(!decompressor.is_null());
    debug_assert!(!out.is_null());
    let buf = std::slice::from_raw_parts_mut(out, len);
    (*decompressor).decompressor.decompress_one(buf).into()
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_decompress_many(
    decompressor: *mut LasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> LazrsResult {
    debug_assert!(!decompressor.is_null());
    debug_assert!(!out.is_null());
    let buf = std::slice::from_raw_parts_mut(out, len);
    (*decompressor).decompressor.decompress_many(buf).into()
}

enum CDest<'a> {
    Memory(Cursor<&'a mut [u8]>),
    File(CFile),
}

impl<'a> std::io::Write for CDest<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            CDest::Memory(cursor) => cursor.write(buf),
            CDest::File(file) => file.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            CDest::File(file) => file.flush(),
            CDest::Memory(cursor) => cursor.flush(),
        }
    }
}

impl<'a> Seek for CDest<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            CDest::Memory(cursor) => cursor.seek(pos),
            CDest::File(file) => file.seek(pos),
        }
    }
}

pub struct LasZipCompressor {
    compressor: laz::LasZipCompressor<'static, CDest<'static>>,
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_compress_new_for_point_format(
    c_compressor: *mut *mut LasZipCompressor,
    file: *mut libc::FILE,
    point_format_id: u8,
    num_extra_bytes: u16,
) -> LazrsResult {
    if c_compressor.is_null() || file.is_null() {
        return LazrsResult::LAZRS_OTHER;
    }
    let items =
        laz::LazItemRecordBuilder::default_for_point_format_id(point_format_id, num_extra_bytes);
    let items = match items {
        Ok(items) => items,
        Err(err) => return err.into(),
    };
    let laz_vlr = laz::LazVlr::from_laz_items(items);
    let dest = CDest::File(CFile {
        fh: NonNull::new_unchecked(file),
    });

    match laz::LasZipCompressor::new(dest, laz_vlr) {
        Ok(compressor) => {
            let compressor = Box::new(LasZipCompressor { compressor });
            *c_compressor = Box::into_raw(compressor);
            LazrsResult::LAZRS_OK
        }
        Err(error) => {
            *c_compressor = std::ptr::null_mut::<LasZipCompressor>();
            error.into()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_compress_one(
    compressor: *mut LasZipCompressor,
    data: *const u8,
    size: usize,
) -> LazrsResult {
    debug_assert!(!compressor.is_null());
    debug_assert!(!data.is_null());
    let slice = std::slice::from_raw_parts(data, size);
    (*compressor).compressor.compress_one(slice).into()
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_compress_many(
    compressor: *mut LasZipCompressor,
    data: *const u8,
    size: usize,
) -> LazrsResult {
    debug_assert!(!compressor.is_null());
    debug_assert!(!data.is_null());
    let slice = std::slice::from_raw_parts(data, size);
    (*compressor).compressor.compress_many(slice).into()
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_done(compressor: *mut LasZipCompressor) -> LazrsResult {
    if compressor.is_null() {
        return LazrsResult::LAZRS_OTHER;
    }
    (*compressor).compressor.done().into()
}

/// Deletes the compressor
///
/// compressor can be NULL
#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_delete(compressor: *mut LasZipCompressor) {
    if !compressor.is_null() {
        Box::from_raw(compressor);
    }
}
