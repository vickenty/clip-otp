use libc;
use std::convert::TryInto;
use std::io::Result;
use std::os::unix::io::AsRawFd;

pub fn wait_with_timeout<T: AsRawFd>(fd: &T, timeout_ms: i32) -> Result<()> {
    let fd = fd.as_raw_fd();

    let pollfd = &mut [libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    }][..];

    let rc = unsafe {
        libc::poll(
            pollfd.as_mut_ptr(),
            pollfd.len().try_into().unwrap(),
            timeout_ms as _,
        )
    };

    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}
