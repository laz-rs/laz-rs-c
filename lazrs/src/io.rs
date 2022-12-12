use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom, Write};
use std::ptr::NonNull;

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

#[derive(Debug)]
pub enum CSource<'a> {
    Memory(Cursor<&'a [u8]>),
    CFile(CFile),
    File(BufReader<File>),
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
        }
    }
}

impl<'a> Seek for CSource<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            CSource::Memory(cursor) => cursor.seek(pos),
            CSource::CFile(file) => file.seek(pos),
            CSource::File(file) => file.seek(pos),
        }
    }
}

pub(crate) enum CDest<'a> {
    Memory(Cursor<&'a mut [u8]>),
    CFile(CFile),
}

impl<'a> std::io::Write for CDest<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            CDest::Memory(cursor) => cursor.write(buf),
            CDest::CFile(file) => file.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            CDest::CFile(file) => file.flush(),
            CDest::Memory(cursor) => cursor.flush(),
        }
    }
}

impl<'a> Seek for CDest<'a> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            CDest::Memory(cursor) => cursor.seek(pos),
            CDest::CFile(file) => file.seek(pos),
        }
    }
}
