#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

mod io;

use laz;
use laz::{LasZipError, LazItem};
use libc;
use std::io::{Cursor};
use std::ptr::NonNull;

use io::{CFile, CDest};
use crate::io::CSource;


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

    let cfile = CFile::new_unchecked(fh);

    match laz::LasZipDecompressor::new(CSource::File(cfile), vlr) {
        Ok(d) => {
            *decompressor = Box::into_raw(Box::new(LasZipDecompressor { decompressor: d }));
            LazrsResult::LAZRS_OK
        }
        Err(error) => {
            *decompressor = std::ptr::null_mut::<LasZipDecompressor>();
            error.into()
        }
    }
}

/// Creates a new decompressor that decompresses data from the
/// given buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompressor_new_buffer(
    decompressor: *mut *mut LasZipDecompressor,
    data: *const u8,
    size: usize,
    laszip_vlr_record_data: *const u8,
    record_data_len: u16,
) -> LazrsResult {
    if decompressor.is_null() || data.is_null() {
        return LazrsResult::LAZRS_OTHER;
    }
    let vlr_data = std::slice::from_raw_parts(laszip_vlr_record_data, usize::from(record_data_len));
    let vlr = match laz::LazVlr::from_buffer(vlr_data) {
        Ok(vlr) => vlr,
        Err(error) => {
            return error.into();
        }
    };

    let source = CSource::Memory {
        0: Cursor::new(std::slice::from_raw_parts(data, size)),
    };
    match laz::LasZipDecompressor::new(source, vlr) {
        Ok(d) => {
            *decompressor = Box::into_raw(Box::new(LasZipDecompressor { decompressor: d }));
            LazrsResult::LAZRS_OK
        }
        Err(error) => {
            *decompressor = std::ptr::null_mut::<LasZipDecompressor>();
            error.into()
        }
    }
}

/// frees the memory for the decompressor
///
/// @decompressor can be NULL (no-op)
#[no_mangle]
pub unsafe extern "C" fn lazrs_delete_decompressor(decompressor: *mut LasZipDecompressor) {
    if !decompressor.is_null() {
        Box::from_raw(decompressor);
    }
}

/// Decompresses one point from the input and write its LAS data to the out buffer
///
/// @decompressor: the decompressor, must not be NULL
/// @out: out buffer that will received the decompressed LAS point
/// @len: size of the output buffer
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

/// Decompresses many (one or more) points from the input and write its LAS data to the out buffer
///
/// @decompressor: the decompressor, must not be NULL
/// @out: out buffer that will received the decompressed LAS point(s)
/// @len: size of the output buffer
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

pub struct LasZipCompressor {
    compressor: laz::LasZipCompressor<'static, CDest<'static>>,
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_new_for_point_format(
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
    let dest = CDest::File(CFile::new_unchecked(file));

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

/// Compresses one point
///
/// @compressor: the compressor, must not be NULL
/// @data: pointer to point buffer to be compressed, the bytes must be the same as the LAS spec
/// @size: size of the point buffer
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

/// Compresses many points
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

/// Tells the compressor that is it done compressing points
///
/// @compressor cannot be NULL
#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_done(compressor: *mut LasZipCompressor) -> LazrsResult {
    debug_assert!(!compressor.is_null());
    (*compressor).compressor.done().into()
}

/// Deletes the compressor
///
/// @compressor can be NULL (no-op)
#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_delete(compressor: *mut LasZipCompressor) {
    if !compressor.is_null() {
        Box::from_raw(compressor);
    }
}
