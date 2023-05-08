use std::io::{Read, Write};

pub trait Term: Write + Read {
    fn forward_inputs(&mut self, other: &mut impl Write) -> std::io::Result<()> {
        let mut buf = [0; 256];
        let size = self.read(&mut buf)?;
        let data = &buf[0..size];
        other.write_all(data)
    }

    fn gather_outputs(&mut self, other: &mut impl Read) -> std::io::Result<usize> {
        let mut buf = [0; 256];
        match other.read(&mut buf) {
            Ok(size) => {
                let data = &buf[0..size];
                self.write_all(data)?;
                Ok(size)
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            err @ Err(_) => err,
        }
    }
}

pub trait WinSizeExt {
    fn get_term_size(&self) -> std::io::Result<libc::winsize>;
    fn set_term_size(&self, win: &libc::winsize) -> std::io::Result<()>;
}

impl WinSizeExt for std::os::fd::RawFd {
    fn get_term_size(&self) -> std::io::Result<libc::winsize> {
        let mut win = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let ret = unsafe { libc::ioctl(*self, libc::TIOCGWINSZ, &mut win as *mut _) };
        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(win)
    }

    fn set_term_size(&self, win: &libc::winsize) -> std::io::Result<()> {
        let ret = unsafe { libc::ioctl(*self, libc::TIOCSWINSZ, win as *const _) };
        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}
