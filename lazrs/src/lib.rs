#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

mod io;

use laz::LasZipError;
use libc;
use std::convert::TryInto;
use std::ffi::CString;
use std::io::{Seek, SeekFrom};
use std::panic::catch_unwind;
use std::panic::{self, AssertUnwindSafe};

use crate::io::{CSource, CustomDest, CustomSource};
use crate::Lazrs_Result::LAZRS_IO_ERROR;
use io::{CDest, CFile};

// enum LastError {
//     Laz(laz::LasZipError),
//     Panic(Box<dyn Any + Send>),
// }
//
// thread_local! {
//     static LAST_ERROR: RefCell<Option<LastError>> = RefCell::new(None);
// }

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Lazrs_Result {
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

#[no_mangle]
pub unsafe extern "C" fn lazrs_fprint_result(res: Lazrs_Result, stream: *mut libc::FILE) {
    let debug_repr = format!("{:?}\n", res);

    let s = CString::new(debug_repr).unwrap();
    libc::fprintf(stream, s.as_c_str().as_ptr());
}

impl From<laz::LasZipError> for Lazrs_Result {
    fn from(e: LasZipError) -> Self {
        match e {
            LasZipError::UnknownLazItem(_) => Lazrs_Result::LAZRS_UNKNOWN_LAZ_ITEM,
            LasZipError::UnsupportedLazItemVersion(_, _) => {
                Lazrs_Result::LAZRS_UNKNOWN_LAZ_ITEM_VERSION
            }
            LasZipError::UnknownCompressorType(_) => Lazrs_Result::LAZRS_UNKNOWN_COMPRESSOR_TYPE,
            LasZipError::UnsupportedCompressorType(_) => {
                Lazrs_Result::LAZRS_UNSUPPORTED_COMPRESSOR_TYPE
            }
            LasZipError::UnsupportedPointFormat(_) => Lazrs_Result::LAZRS_UNSUPPORTED_POINT_FORMAT,
            LasZipError::IoError(_) => Lazrs_Result::LAZRS_IO_ERROR,
            LasZipError::MissingChunkTable => Lazrs_Result::LAZRS_MISSING_CHUNK_TABLE,
            _ => Lazrs_Result::LAZRS_OTHER,
        }
    }
}

impl From<Result<(), laz::LasZipError>> for Lazrs_Result {
    fn from(r: Result<(), LasZipError>) -> Self {
        match r {
            Ok(_) => Lazrs_Result::LAZRS_OK,
            Err(e) => e.into(),
        }
    }
}

impl From<std::io::Result<()>> for Lazrs_Result {
    fn from(r: std::io::Result<()>) -> Self {
        match r {
            Ok(_) => Lazrs_Result::LAZRS_OK,
            Err(_) => Lazrs_Result::LAZRS_IO_ERROR,
        }
    }
}

/// Simple struct representing a non-mutable byte buffer
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Lazrs_Buffer {
    data: *const u8,
    len: usize,
}

/// The different LAZ source type supported
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum Lazrs_SourceType {
    /// The source is an in memory buffer
    LAZRS_SOURCE_BUFFER,
    // The source is a C `FILE` pointer
    LAZRS_SOURCE_CFILE,
    // The source is a filename that lazrs will open
    LAZRS_SOURCE_FNAME,
    // The source is something custom
    LAZRS_SOURCE_CUSTOM,
}

/// Union of possible sources
#[repr(C)]
#[derive(Copy, Clone)]
pub union Lazrs_Source {
    file: *mut libc::FILE,
    buffer: Lazrs_Buffer,
    custom: CustomSource,
}

/// The needed parameters to create a LasZipDecompressor
#[repr(C)]
pub struct Lazrs_DecompressorParams {
    source_type: Lazrs_SourceType,
    source: Lazrs_Source,
    // TODO explain what is offset is or remove it
    source_offset: u64,
    laszip_vlr: Lazrs_Buffer,
}

/// A single-threaded sequential
pub struct Lazrs_SeqLasZipDecompressor(laz::LasZipDecompressor<'static, CSource<'static>>);

/// Creates a new sequential that decompresses data from the given file
///
/// If an error occurs, the returned result will be something other that LAZRS_OK
/// and the sequential will be set to NULL.
#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_laszip_decompressor_new(
    params: Lazrs_DecompressorParams,
    decompressor: *mut *mut Lazrs_SeqLasZipDecompressor,
) -> Lazrs_Result {
    debug_assert!(!decompressor.is_null());

    let call_result = panic::catch_unwind(AssertUnwindSafe(|| {
        let vlr_data =
            std::slice::from_raw_parts(params.laszip_vlr.data, usize::from(params.laszip_vlr.len));
        let vlr = match laz::LazVlr::from_buffer(vlr_data) {
            Ok(vlr) => vlr,
            Err(error) => {
                return error.into();
            }
        };

        let mut csource = match CSource::from_c_source(params.source_type, params.source) {
            Ok(v) => v,
            Err(result) => return result,
        };

        if let Err(_error) = csource.seek(SeekFrom::Start(params.source_offset)) {
            return LAZRS_IO_ERROR;
        }

        match laz::LasZipDecompressor::new(csource, vlr) {
            Ok(d) => {
                *decompressor = Box::into_raw(Box::new(Lazrs_SeqLasZipDecompressor(d)));
                Lazrs_Result::LAZRS_OK
            }
            Err(error) => {
                *decompressor = std::ptr::null_mut::<Lazrs_SeqLasZipDecompressor>();
                error.into()
            }
        }
    }));

    match call_result {
        Ok(r) => r,
        Err(_) => Lazrs_Result::LAZRS_OTHER,
    }
}

/// frees the memory for the sequential
///
/// @sequential can be NULL (no-op)
#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_laszip_decompressor_delete(
    decompressor: *mut Lazrs_SeqLasZipDecompressor,
) {
    if !decompressor.is_null() {
        let _ = Box::from_raw(decompressor);
    }
}

/// Decompresses one point from the input and write its LAS data to the out buffer
///
/// @sequential: the sequential, must not be NULL
/// @out: out buffer that will received the decompressed LAS point
/// @len: size of the output buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_laszip_decompressor_decompress_one(
    decompressor: *mut Lazrs_SeqLasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> Lazrs_Result {
    debug_assert!(!decompressor.is_null());
    debug_assert!(!out.is_null());

    let mut decompressor = AssertUnwindSafe(&mut *decompressor);
    let call_result = catch_unwind(move || -> Lazrs_Result {
        let buf = std::slice::from_raw_parts_mut(out, len);
        (*decompressor).0.decompress_one(buf).into()
    });
    match call_result {
        Ok(r) => r,
        Err(_) => Lazrs_Result::LAZRS_OTHER,
    }
}

/// Decompresses many (one or more) points from the input and write its LAS data to the out buffer
///
/// @sequential: the sequential, must not be NULL
/// @out: out buffer that will received the decompressed LAS point(s)
/// @len: size of the output buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_laszip_decompressor_decompress_many(
    decompressor: *mut Lazrs_SeqLasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> Lazrs_Result {
    debug_assert!(!decompressor.is_null());
    debug_assert!(!out.is_null());
    let mut decompressor = AssertUnwindSafe(&mut *decompressor);

    let call_result = catch_unwind(move || -> Lazrs_Result {
        let buf = std::slice::from_raw_parts_mut(out, len);
        (*decompressor).0.decompress_many(buf).into()
    });

    match call_result {
        Ok(r) => r,
        Err(_) => Lazrs_Result::LAZRS_OTHER,
    }
}

//==================================================================================================

/// A multi-threaded decompressor
#[cfg(feature = "parallel")]
pub struct Lazrs_ParLasZipDecompressor(laz::ParLasZipDecompressor<CSource<'static>>);

/// Creates a new sequential that decompresses data from the given file
///
/// If an error occurs, the returned result will be something other that LAZRS_OK
/// and the sequential will be set to NULL.
#[cfg(feature = "parallel")]
#[no_mangle]
pub unsafe extern "C" fn lazrs_par_laszip_decompressor_new(
    params: Lazrs_DecompressorParams,
    decompressor: *mut *mut Lazrs_ParLasZipDecompressor,
) -> Lazrs_Result {
    debug_assert!(!decompressor.is_null());

    let vlr_data =
        std::slice::from_raw_parts(params.laszip_vlr.data, usize::from(params.laszip_vlr.len));
    let vlr = match laz::LazVlr::from_buffer(vlr_data) {
        Ok(vlr) => vlr,
        Err(error) => {
            return error.into();
        }
    };

    let mut csource = match CSource::from_c_source(params.source_type, params.source) {
        Ok(v) => v,
        Err(result) => return result,
    };

    if let Err(_error) = csource.seek(SeekFrom::Start(params.source_offset)) {
        return LAZRS_IO_ERROR;
    }
    match laz::ParLasZipDecompressor::new(csource, vlr) {
        Ok(d) => {
            *decompressor = Box::into_raw(Box::new(Lazrs_ParLasZipDecompressor(d)));
            Lazrs_Result::LAZRS_OK
        }
        Err(error) => {
            *decompressor = std::ptr::null_mut::<Lazrs_ParLasZipDecompressor>();
            error.into()
        }
    }
}

/// frees the memory for the sequential
///
/// @sequential can be NULL (no-op)
#[cfg(feature = "parallel")]
#[no_mangle]
pub unsafe extern "C" fn lazrs_par_laszip_decompressor_delete(
    decompressor: *mut Lazrs_ParLasZipDecompressor,
) {
    if !decompressor.is_null() {
        let _ = Box::from_raw(decompressor);
    }
}

/// Decompresses many (one or more) points from the input and write its LAS data to the out buffer
///
/// @sequential: the sequential, must not be NULL
/// @out: out buffer that will received the decompressed LAS point(s)
/// @len: size of the output buffer
#[cfg(feature = "parallel")]
#[no_mangle]
pub unsafe extern "C" fn lazrs_par_laszip_decompressor_decompress_many(
    decompressor: *mut Lazrs_ParLasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> Lazrs_Result {
    debug_assert!(!decompressor.is_null());
    debug_assert!(!out.is_null());
    let buf = std::slice::from_raw_parts_mut(out, len);
    (*decompressor).0.decompress_many(buf).into()
}

//==================================================================================================

/// A decompressor that can be either single or multi-threaded.
///
/// The choice is done at creation time and cannot be changed midway through the
/// decompression
pub enum Lazrs_LasZipDecompressor {
    sequential(laz::LasZipDecompressor<'static, CSource<'static>>),
    #[cfg(feature = "parallel")]
    parallel(laz::ParLasZipDecompressor<CSource<'static>>),
}

/// Creates a new sequential that decompresses data from the given file
///
/// If an error occurs, the returned result will be something other that LAZRS_OK
/// and the sequential will be set to NULL.
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompressor_new(
    params: Lazrs_DecompressorParams,
    prefer_parallel: bool,
    decompressor: *mut *mut Lazrs_LasZipDecompressor,
) -> Lazrs_Result {
    debug_assert!(!decompressor.is_null());

    let vlr_data =
        std::slice::from_raw_parts(params.laszip_vlr.data, usize::from(params.laszip_vlr.len));
    let vlr = match laz::LazVlr::from_buffer(vlr_data) {
        Ok(vlr) => vlr,
        Err(error) => {
            return error.into();
        }
    };

    let mut csource = match CSource::from_c_source(params.source_type, params.source) {
        Ok(v) => v,
        Err(result) => return result,
    };

    if let Err(_error) = csource.seek(SeekFrom::Start(params.source_offset)) {
        return LAZRS_IO_ERROR;
    }
    #[cfg(feature = "parallel")]
    {
        if prefer_parallel {
            match laz::ParLasZipDecompressor::new(csource, vlr) {
                Ok(d) => {
                    *decompressor = Box::into_raw(Box::new(Lazrs_LasZipDecompressor::parallel(d)));
                    Lazrs_Result::LAZRS_OK
                }
                Err(error) => {
                    *decompressor = std::ptr::null_mut::<Lazrs_LasZipDecompressor>();
                    error.into()
                }
            }
        } else {
            match laz::LasZipDecompressor::new(csource, vlr) {
                Ok(d) => {
                    *decompressor =
                        Box::into_raw(Box::new(Lazrs_LasZipDecompressor::sequential(d)));
                    Lazrs_Result::LAZRS_OK
                }
                Err(error) => {
                    *decompressor = std::ptr::null_mut::<Lazrs_LasZipDecompressor>();
                    error.into()
                }
            }
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = prefer_parallel;
        match laz::LasZipDecompressor::new(csource, vlr) {
            Ok(d) => {
                *decompressor = Box::into_raw(Box::new(Lazrs_LasZipDecompressor::sequential(d)));
                Lazrs_Result::LAZRS_OK
            }
            Err(error) => {
                *decompressor = std::ptr::null_mut::<Lazrs_LasZipDecompressor>();
                error.into()
            }
        }
    }
}

/// frees the memory for the sequential
///
/// @sequential can be NULL (no-op)
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompressor_delete(decompressor: *mut Lazrs_LasZipDecompressor) {
    if !decompressor.is_null() {
        let _ = Box::from_raw(decompressor);
    }
}

/// Decompresses one point from the input and write its LAS data to the out buffer
///
/// @sequential: the sequential, must not be NULL
/// @out: out buffer that will received the decompressed LAS point
/// @len: size of the output buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompressor_decompress_one(
    decompressor: *mut Lazrs_LasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> Lazrs_Result {
    debug_assert!(!decompressor.is_null());
    debug_assert!(!out.is_null());
    let buf = std::slice::from_raw_parts_mut(out, len);
    match *decompressor {
        #[cfg(feature = "parallel")]
        Lazrs_LasZipDecompressor::parallel(ref mut d) => d.decompress_many(buf).into(),
        Lazrs_LasZipDecompressor::sequential(ref mut d) => d.decompress_one(buf).into(),
    }
}

/// Decompresses many (one or more) points from the input and write its LAS data to the out buffer
///
/// @sequential: the sequential, must not be NULL
/// @out: out buffer that will received the decompressed LAS point(s)
/// @len: size of the output buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_decompressor_decompress_many(
    decompressor: *mut Lazrs_LasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) -> Lazrs_Result {
    debug_assert!(!decompressor.is_null());
    debug_assert!(!out.is_null());
    let buf = std::slice::from_raw_parts_mut(out, len);
    match *decompressor {
        #[cfg(feature = "parallel")]
        Lazrs_LasZipDecompressor::parallel(ref mut d) => d.decompress_many(buf).into(),
        Lazrs_LasZipDecompressor::sequential(ref mut d) => d.decompress_many(buf).into(),
    }
}

//==================================================================================================

/// The different LAZ destination type supported
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum Lazrs_DestType {
    LAZRS_DEST_CFILE,
    LAZRS_DEST_CUSTOM,
}

/// Union of possible sources
#[repr(C)]
#[derive(Copy, Clone)]
pub union Lazrs_Dest {
    file: *mut libc::FILE,
    buffer: Lazrs_Buffer,
    custom: CustomDest,
}

#[repr(C)]
pub struct Lazrs_CompressorParams {
    dest_type: Lazrs_DestType,
    dest: Lazrs_Dest,
    point_format_id: u8,
    num_extra_bytes: u16,
}

pub struct Lazrs_SeqLasZipCompressor {
    compressor: laz::LasZipCompressor<'static, CDest>,
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_compressor_new_for_point_format(
    params: Lazrs_CompressorParams,
    c_compressor: *mut *mut Lazrs_SeqLasZipCompressor,
) -> Lazrs_Result {
    if c_compressor.is_null() {
        return Lazrs_Result::LAZRS_OTHER;
    }
    let items = laz::LazItemRecordBuilder::default_for_point_format_id(
        params.point_format_id,
        params.num_extra_bytes,
    );
    let items = match items {
        Ok(items) => items,
        Err(err) => return err.into(),
    };
    let laz_vlr = laz::LazVlr::from_laz_items(items);

    let dest = match params.dest_type {
        Lazrs_DestType::LAZRS_DEST_CFILE => CDest::CFile(CFile::new_unchecked(params.dest.file)),
        Lazrs_DestType::LAZRS_DEST_CUSTOM => CDest::Custom(params.dest.custom),
    };

    match laz::LasZipCompressor::new(dest, laz_vlr) {
        Ok(compressor) => {
            let compressor = Box::new(Lazrs_SeqLasZipCompressor { compressor });
            *c_compressor = Box::into_raw(compressor);
            Lazrs_Result::LAZRS_OK
        }
        Err(error) => {
            *c_compressor = std::ptr::null_mut::<Lazrs_SeqLasZipCompressor>();
            error.into()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_compressor_laszip_vlr_size(
    compressor: *mut Lazrs_SeqLasZipCompressor,
) -> u16 {
    debug_assert!(!compressor.is_null());

    // TODO we should have a data_len() function in laz-rs
    let mut data = Vec::<u8>::new();
    (*compressor).compressor.vlr().write_to(&mut data).unwrap();

    return data.len().try_into().unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_compressor_laszip_vlr_data(
    compressor: *mut Lazrs_SeqLasZipCompressor,
    data: *mut u8,
    size: usize,
) -> Lazrs_Result {
    debug_assert!(!compressor.is_null());

    // TODO having to create this temp data vec is a bit sub optimal
    // but slices don't impl Write
    let mut tmp = Vec::<u8>::new();
    let r = (*compressor).compressor.vlr().write_to(&mut tmp).into();

    std::slice::from_raw_parts_mut(data, size).copy_from_slice(tmp.as_slice());

    r
}

/// Compresses one point
///
/// @compressor: the compressor, must not be NULL
/// @data: pointer to point buffer to be compressed, the bytes must be the same as the LAS spec
/// @size: size of the point buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_compressor_compress_one(
    compressor: *mut Lazrs_SeqLasZipCompressor,
    data: *const u8,
    size: usize,
) -> Lazrs_Result {
    debug_assert!(!compressor.is_null());
    debug_assert!(!data.is_null());
    let slice = std::slice::from_raw_parts(data, size);
    (*compressor).compressor.compress_one(slice).into()
}

/// Compresses many points
#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_compressor_compress_many(
    compressor: *mut Lazrs_SeqLasZipCompressor,
    data: *const u8,
    size: usize,
) -> Lazrs_Result {
    debug_assert!(!compressor.is_null());
    debug_assert!(!data.is_null());
    let slice = std::slice::from_raw_parts(data, size);
    (*compressor).compressor.compress_many(slice).into()
}

/// Tells the compressor that is it done compressing points
///
/// @compressor cannot be NULL
#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_compressor_done(
    compressor: *mut Lazrs_SeqLasZipCompressor,
) -> Lazrs_Result {
    debug_assert!(!compressor.is_null());
    (*compressor).compressor.done().into()
}

/// Deletes the compressor
///
/// @compressor can be NULL (no-op)
#[no_mangle]
pub unsafe extern "C" fn lazrs_seq_compressor_delete(compressor: *mut Lazrs_SeqLasZipCompressor) {
    if !compressor.is_null() {
        let _ = Box::from_raw(compressor);
    }
}

//==================================================================================================

/// A compressor that can be either single or multi-threaded.
///
/// The choice is done at creation time and cannot be changed midway through the
/// compression
pub enum Lazrs_LasZipCompressor {
    sequential(laz::LasZipCompressor<'static, CDest>),
    #[cfg(feature = "parallel")]
    parallel(laz::ParLasZipCompressor<CDest>),
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_new_for_point_format(
    params: Lazrs_CompressorParams,
    prefer_parallel: bool,
    c_compressor: *mut *mut Lazrs_LasZipCompressor,
) -> Lazrs_Result {
    if c_compressor.is_null() {
        return Lazrs_Result::LAZRS_OTHER;
    }
    let items = laz::LazItemRecordBuilder::default_for_point_format_id(
        params.point_format_id,
        params.num_extra_bytes,
    );
    let items = match items {
        Ok(items) => items,
        Err(err) => return err.into(),
    };
    let laz_vlr = laz::LazVlr::from_laz_items(items);

    let dest = match params.dest_type {
        Lazrs_DestType::LAZRS_DEST_CFILE => CDest::CFile(CFile::new_unchecked(params.dest.file)),
        Lazrs_DestType::LAZRS_DEST_CUSTOM => CDest::Custom(params.dest.custom),
    };

    #[cfg(feature = "parallel")]
    if prefer_parallel {
        match laz::ParLasZipCompressor::new(dest, laz_vlr) {
            Ok(compressor) => {
                let compressor = Box::new(Lazrs_LasZipCompressor::parallel(compressor));
                *c_compressor = Box::into_raw(compressor);
                Lazrs_Result::LAZRS_OK
            }
            Err(error) => {
                *c_compressor = std::ptr::null_mut::<Lazrs_LasZipCompressor>();
                error.into()
            }
        }
    } else {
        match laz::LasZipCompressor::new(dest, laz_vlr) {
            Ok(compressor) => {
                let compressor = Box::new(Lazrs_LasZipCompressor::sequential(compressor));
                *c_compressor = Box::into_raw(compressor);
                Lazrs_Result::LAZRS_OK
            }
            Err(error) => {
                *c_compressor = std::ptr::null_mut::<Lazrs_LasZipCompressor>();
                error.into()
            }
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = prefer_parallel;
        match laz::LasZipCompressor::new(dest, laz_vlr) {
            Ok(compressor) => {
                let compressor = Box::new(Lazrs_LasZipCompressor::sequential(compressor));
                *c_compressor = Box::into_raw(compressor);
                Lazrs_Result::LAZRS_OK
            }
            Err(error) => {
                *c_compressor = std::ptr::null_mut::<Lazrs_LasZipCompressor>();
                error.into()
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_laszip_vlr_size(
    compressor: *mut Lazrs_LasZipCompressor,
) -> u16 {
    debug_assert!(!compressor.is_null());

    // TODO we should have a data_len() function in laz-rs
    let mut data = Vec::<u8>::new();
    match &*compressor {
        Lazrs_LasZipCompressor::sequential(compressor) => {
            compressor.vlr().write_to(&mut data).unwrap();
        }
        #[cfg(feature = "parallel")]
        Lazrs_LasZipCompressor::parallel(compressor) => {
            compressor.vlr().write_to(&mut data).unwrap();
        }
    }

    return data.len().try_into().unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_laszip_vlr_data(
    compressor: *mut Lazrs_LasZipCompressor,
    data: *mut u8,
    size: usize,
) -> Lazrs_Result {
    debug_assert!(!compressor.is_null());

    // TODO having to create this temp data vec is a bit sub optimal
    // but slices don't impl Write
    let mut tmp = Vec::<u8>::new();
    let r = match &*compressor {
        Lazrs_LasZipCompressor::sequential(compressor) => {
            compressor.vlr().write_to(&mut tmp).into()
        }
        #[cfg(feature = "parallel")]
        Lazrs_LasZipCompressor::parallel(compressor) => compressor.vlr().write_to(&mut tmp).into(),
    };

    if r == Lazrs_Result::LAZRS_OK {
        std::slice::from_raw_parts_mut(data, size).copy_from_slice(tmp.as_slice());
    }

    r
}

/// Compresses one point
///
/// @compressor: the compressor, must not be NULL
/// @data: pointer to point buffer to be compressed, the bytes must be the same as the LAS spec
/// @size: size of the point buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_compress_one(
    compressor: *mut Lazrs_LasZipCompressor,
    data: *const u8,
    size: usize,
) -> Lazrs_Result {
    debug_assert!(!compressor.is_null());
    debug_assert!(!data.is_null());
    let slice = std::slice::from_raw_parts(data, size);
    match &mut *compressor {
        Lazrs_LasZipCompressor::sequential(compressor) => compressor.compress_one(slice).into(),
        #[cfg(feature = "parallel")]
        Lazrs_LasZipCompressor::parallel(compressor) => compressor.compress_many(slice).into(),
    }
}

/// Compresses many points
///
/// @compressor: the compressor, must not be NULL
/// @data: pointer to point buffer to be compressed, the bytes must be the same as the LAS spec
/// @size: size of the point buffer
#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_compress_many(
    compressor: *mut Lazrs_LasZipCompressor,
    data: *const u8,
    size: usize,
) -> Lazrs_Result {
    debug_assert!(!compressor.is_null());
    debug_assert!(!data.is_null());
    let slice = std::slice::from_raw_parts(data, size);
    match &mut *compressor {
        Lazrs_LasZipCompressor::sequential(compressor) => compressor.compress_many(slice).into(),
        #[cfg(feature = "parallel")]
        Lazrs_LasZipCompressor::parallel(compressor) => compressor.compress_many(slice).into(),
    }
}

/// Tells the compressor that is it done compressing points
///
/// @compressor cannot be NULL
#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_done(
    compressor: *mut Lazrs_LasZipCompressor,
) -> Lazrs_Result {
    debug_assert!(!compressor.is_null());
    match &mut *compressor {
        Lazrs_LasZipCompressor::sequential(compressor) => compressor.done().into(),
        #[cfg(feature = "parallel")]
        Lazrs_LasZipCompressor::parallel(compressor) => compressor.done().into(),
    }
}

/// Deletes the compressor
///
/// @compressor can be NULL (no-op)
#[no_mangle]
pub unsafe extern "C" fn lazrs_compressor_delete(compressor: *mut Lazrs_LasZipCompressor) {
    if !compressor.is_null() {
        let _ = Box::from_raw(compressor);
    }
}
