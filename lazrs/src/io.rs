use crate::Lazrs_SourceType;
use libc::c_int;
use std::convert::TryInto;
use std::ffi::c_void;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom, Write};
use std::ptr::NonNull;

fn seek_from_to_c_whence(seek_from: SeekFrom) -> (i64, c_int) {
    match seek_from {
        SeekFrom::Start(pos) => {
            assert!(pos < i64::MAX as u64);
            (pos as i64, libc::SEEK_SET)
        }
        SeekFrom::End(pos) => (pos, libc::SEEK_END),
        SeekFrom::Current(pos) => (pos, libc::SEEK_CUR),
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CustomSource {
    pub user_data: *mut c_void,
    pub read_fn: unsafe extern "C" fn(user_data: *mut c_void, n: u64, out_buffer: *mut u8) -> u64,
    pub seek_fn: unsafe extern "C" fn(user_data: *mut c_void, pos: i64, from: c_int) -> c_int,
    pub tell_fn: unsafe extern "C" fn(user_data: *mut c_void) -> u64,
}

unsafe impl Send for CustomSource {}
unsafe impl Sync for CustomSource {}

impl Read for CustomSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().try_into().unwrap();
        assert!(!self.user_data.is_null());

        let n_read = unsafe {
            // SAFETY
            // the ptr is not null
            (self.read_fn)(self.user_data, n, buf.as_mut_ptr())
        };
        Ok(n_read.try_into().unwrap())
    }
}

impl Seek for CustomSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let (pos, whence) = seek_from_to_c_whence(pos);
        let ret = unsafe { (self.seek_fn)(self.user_data, pos, whence) };

        if ret != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to seek".to_string(),
            ));
        }
        let position = unsafe { (self.tell_fn)(self.user_data) };

        Ok(position as u64)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CFile {
    fh: NonNull<libc::FILE>,
}

impl CFile {
    /// file must not be null, and it is not checked
    pub(crate) unsafe fn new_unchecked(file: *mut libc::FILE) -> Self {
        debug_assert!(!file.is_null());
        CFile {
            fh: NonNull::new_unchecked(file),
        }
    }
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
            let (pos, whence) = seek_from_to_c_whence(pos);

            if pos != 0 && whence != libc::SEEK_CUR {
                let result = libc::fseek(self.fh.as_ptr(), pos.try_into().unwrap(), whence);
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

#[derive(Debug)]
pub enum CSource<'a> {
    Memory(Cursor<&'a [u8]>),
    CFile(CFile),
    File(BufReader<File>),
    Custom(CustomSource),
}

impl<'a> CSource<'a> {
    pub(crate) unsafe fn from_c_source(
        source_type: crate::Lazrs_SourceType,
        source: crate::Lazrs_Source,
    ) -> Result<Self, crate::Lazrs_Result> {
        let csource = match source_type {
            crate::Lazrs_SourceType::LAZRS_SOURCE_BUFFER => CSource::Memory(Cursor::new(
                std::slice::from_raw_parts(source.buffer.data, source.buffer.len),
            )),
            crate::Lazrs_SourceType::LAZRS_SOURCE_CFILE => {
                CSource::CFile(CFile::new_unchecked(source.file))
            }
            crate::Lazrs_SourceType::LAZRS_SOURCE_FNAME => {
                match std::str::from_utf8(std::slice::from_raw_parts(
                    source.buffer.data,
                    source.buffer.len,
                )) {
                    Ok(fname) => match File::open(std::path::Path::new(fname)) {
                        Ok(f) => CSource::File(BufReader::new(f)),
                        Err(_error) => {
                            return Err(crate::Lazrs_Result::LAZRS_IO_ERROR);
                        }
                    },
                    Err(_error) => {
                        return Err(crate::Lazrs_Result::LAZRS_IO_ERROR);
                    }
                }
            }
            Lazrs_SourceType::LAZRS_SOURCE_CUSTOM => Self::Custom(source.custom),
        };
        Ok(csource)
    }
}

impl<'a> Read for CSource<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            CSource::Memory(cursor) => cursor.read(buf),
            CSource::CFile(file) => file.read(buf),
            CSource::File(file) => file.read(buf),
            CSource::Custom(custom) => custom.read(buf),
        }
    }
}

impl<'a> Seek for CSource<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            CSource::Memory(cursor) => cursor.seek(pos),
            CSource::CFile(file) => file.seek(pos),
            CSource::File(file) => file.seek(pos),
            CSource::Custom(custom) => custom.seek(pos),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CustomDest {
    pub user_data: *mut c_void,
    pub write_fn: unsafe extern "C" fn(user_data: *mut c_void, buffer: *const u8, n: u64) -> u64,
    pub flush_fn: unsafe extern "C" fn(user_data: *mut c_void) -> c_int,
    pub seek_fn: unsafe extern "C" fn(user_data: *mut c_void, pos: i64, from: c_int) -> c_int,
    pub tell_fn: unsafe extern "C" fn(user_data: *mut c_void) -> u64,
}

unsafe impl Send for CustomDest {}
unsafe impl Sync for CustomDest {}

impl Write for CustomDest {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        assert!(!self.user_data.is_null());

        let ptr = buf.as_ptr();
        let size = buf.len();

        let n_written = unsafe { (self.write_fn)(self.user_data, ptr, size.try_into().unwrap()) };

        Ok(n_written.try_into().unwrap())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        assert!(!self.user_data.is_null());

        let r = unsafe { (self.flush_fn)(self.user_data) };

        if r != 0 {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to flush".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

impl Seek for CustomDest {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let (pos, whence) = seek_from_to_c_whence(pos);
        let ret = unsafe { (self.seek_fn)(self.user_data, pos, whence) };

        if ret != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to seek".to_string(),
            ));
        }
        let position = unsafe { (self.tell_fn)(self.user_data) };

        Ok(position as u64)
    }
}

pub enum CDest {
    CFile(CFile),
    Custom(CustomDest),
}

impl std::io::Write for CDest {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            CDest::CFile(file) => file.write(buf),
            CDest::Custom(custom) => custom.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            CDest::CFile(file) => file.flush(),
            CDest::Custom(custom) => custom.flush(),
        }
    }
}

impl Seek for CDest {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            CDest::CFile(file) => file.seek(pos),
            CDest::Custom(custom) => custom.seek(pos),
        }
    }
}
