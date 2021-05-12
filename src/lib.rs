use laz;
use libc;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::ptr::NonNull;

#[derive(Copy, Clone)]
struct CFile {
    fh: NonNull<libc::FILE>,
}

unsafe impl Send for CFile {}

impl Read for CFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        println!("CFile::Read {}", buf.len());
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
        println!("CFile::Seek: {:?}", pos);
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
                let result = dbg!(libc::fseek(self.fh.as_ptr(), pos, whence));
                if result != 0 {
                    return Err(std::io::Error::from_raw_os_error(result));
                }
            }
            let position = libc::ftell(self.fh.as_ptr());
            Ok(position as u64)
        }
    }
}

// pub struct CustomSource {
//     src: *mut libc::c_void,
//     read_fn: extern "C" fn(src: *mut libc::c_void, buf: *mut char, len: usize) -> usize,
//     seek_fn: extern "C" fn(src: *mut libc::c_void, offset: i64, whence: i32) -> usize,
// }

enum CSource<'a> {
    Memory(Cursor<&'a [u8]>),
    File(CFile),
    // Custom(CustomSource)
}

impl<'a> Read for CSource<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            CSource::Memory(cursor) => cursor.read(buf),
            CSource::File(file) => file.read(buf), // CSource::Custom(src) => {
                                                   //     todo!()
                                                   // }
        }
    }
}

impl<'a> Seek for CSource<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            CSource::Memory(cursor) => cursor.seek(pos),
            CSource::File(file) => file.seek(pos), // CSource::Custom(src) => {
                                                   //     todo!()
                                                   // }
        }
    }
}

pub struct LasZipDecompressor {
    decompressor: laz::LasZipDecompressor<'static, CSource<'static>>,
}

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
    let mut cfile = CFile {
        fh: NonNull::new(fh).unwrap(),
    };
    let dbox = Box::new(LasZipDecompressor {
        decompressor: laz::LasZipDecompressor::new(CSource::File(cfile), vlr).unwrap(),
    });
    Box::into_raw(dbox)
}

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
) {
    if decompressor.is_null() || out.is_null() {
        return;
    }
    let buf = std::slice::from_raw_parts_mut(out, len);
    (*decompressor).decompressor.decompress_one(buf).unwrap();
}

#[no_mangle]
pub unsafe extern "C" fn lazrs_decompress_many(
    decompressor: *mut LasZipDecompressor,
    out: *mut u8,
    len: libc::size_t,
) {
    if decompressor.is_null() || out.is_null() {
        return;
    }
    let buf = std::slice::from_raw_parts_mut(out, len);
    (*decompressor).decompressor.decompress_many(buf).unwrap();
}


// pub struct LasZipCompressor {
//     compressor: laz::LasZipCompressor<'static, CFile>
// }
