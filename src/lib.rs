#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use laz;
use libc;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::ptr::NonNull;
use laz::LasZipError;
use crate::LazrsResult::LAZRS_OTHER;

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
            LasZipError::UnsupportedLazItemVersion(_, _) => LazrsResult::LAZRS_UNKNOWN_LAZ_ITEM_VERSION,
            LasZipError::UnknownCompressorType(_) => LazrsResult::LAZRS_UNKNOWN_COMPRESSOR_TYPE,
            LasZipError::UnsupportedCompressorType(_) => LazrsResult::LAZRS_UNSUPPORTED_COMPRESSOR_TYPE,
            LasZipError::UnsupportedPointFormat(_) => LazrsResult::LAZRS_UNSUPPORTED_POINT_FORMAT,
            LasZipError::IoError(_) => LazrsResult::LAZRS_IO_ERROR,
            LasZipError::MissingChunkTable => LazrsResult::LAZRS_MISSING_CHUNK_TABLE,
            _ => LazrsResult::LAZRS_OTHER
        }
    }
}

impl From<Result<(), laz::LasZipError>> for LazrsResult {
    fn from(r: Result<(), LasZipError>) -> Self {
        match r {
            Ok(_) => LazrsResult::LAZRS_OK,
            Err(e) => e.into()
        }
    }
}

impl From<std::io::Result<()>> for LazrsResult {
    fn from(r: std::io::Result<()>) -> Self {
        match r {
            Ok(_) => LazrsResult::LAZRS_OK,
            Err(_) => LazrsResult::LAZRS_IO_ERROR
        }
    }
}

pub struct LasZipDecompressor {
    decompressor: laz::LasZipDecompressor<'static, CSource<'static>>,
}


/// Creates a new decompressor that decompresses data from the given file
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompressor_new_file(
    fh: *mut libc::FILE,
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
    let cfile = CFile {
        fh: NonNull::new(fh).unwrap(),
    };
    let dbox = Box::new(LasZipDecompressor {
        decompressor: laz::LasZipDecompressor::new(CSource::File(cfile), vlr).unwrap(),
    });
    Box::into_raw(dbox)
}

/// Creates a new decompressor that decompresses data from the
/// given buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompress_new_buffer(
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
        0: Cursor::new(std::slice::from_raw_parts(data, size))
    };
    let dbox = Box::new(LasZipDecompressor {
        decompressor: laz::LasZipDecompressor::new(source, vlr).unwrap(),
    });
    Box::into_raw(dbox)
}

/// Deletes the decompressor
#[no_mangle]
pub unsafe extern "C" fn lazrs_delete_decompressor(decompressor: *mut LasZipDecompressor) {
    if decompressor.is_null() {
        return;
    }
    Box::from_raw(decompressor);
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_decompress_one(
    decompressor: *mut LasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> LazrsResult {
    if decompressor.is_null() || out.is_null() {
        return LAZRS_OTHER;
    }
    let buf = std::slice::from_raw_parts_mut(out, len);
    (*decompressor).decompressor.decompress_one(buf).into()
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_decompress_many(
    decompressor: *mut LasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> LazrsResult {
    if decompressor.is_null() || out.is_null() {
        return LAZRS_OTHER;
    }
    let buf = std::slice::from_raw_parts_mut(out, len);
    (*decompressor).decompressor.decompress_many(buf).into()
}
