use libc;
use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::sync::LazyLock;
use std::sync::Mutex;

type OpenFn = unsafe extern "C" fn(*const libc::c_char, libc::c_int) -> libc::c_int;
type CloseFn = unsafe extern "C" fn(libc::c_int) -> libc::c_int;
type ReadFn = unsafe extern "C" fn(libc::c_int, *mut c_void, libc::size_t) -> libc::ssize_t;

static SECRET_FILE_NAME: &str = "secret.txt";
static SECRET_CONTENT: &str = "Secret!";

// Track the file descriptors and their current read position.
static mut SECRET_DESCRIPTOR_TO_POSITION: LazyLock<Mutex<HashMap<libc::c_int, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[no_mangle]
pub extern "C" fn open(path: *const libc::c_char, oflag: libc::c_int) -> libc::c_int {
    // Get the original open function
    let original_open: OpenFn = unsafe {
        let open_name = CString::new("open").unwrap();
        let open_ptr = libc::dlsym(libc::RTLD_NEXT, open_name.as_ptr());
        std::mem::transmute(open_ptr)
    };

    // Call the original open function
    let fd = unsafe { original_open(path, oflag) };
    // Make a secret file secret only in read mode.
    if fd < 0 || oflag != libc::O_RDONLY {
        return fd;
    }

    let file_path = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let file_name = file_path.split("/").last().unwrap();
    if file_name == SECRET_FILE_NAME {
        unsafe { SECRET_DESCRIPTOR_TO_POSITION.lock().unwrap().insert(fd, 0) };
    }

    return fd;
}

#[no_mangle]
pub extern "C" fn close(fd: libc::c_int) -> libc::c_int {
    // Get the original close function
    let original_close: CloseFn = unsafe {
        let close_name = CString::new("close").unwrap();
        let close_ptr = libc::dlsym(libc::RTLD_NEXT, close_name.as_ptr());
        std::mem::transmute(close_ptr)
    };
    let result = unsafe { original_close(fd) };
    if fd < 0 {
        return result;
    }
    unsafe { SECRET_DESCRIPTOR_TO_POSITION.lock().unwrap().remove(&fd) };
    return result;
}

#[no_mangle]
pub extern "C" fn read(
    fd: libc::c_int,
    buf: *mut libc::c_void,
    count: libc::size_t,
) -> libc::ssize_t {
    // Get the original read function
    let original_read: ReadFn = unsafe {
        let read_name = CString::new("read").unwrap();
        let read_ptr = libc::dlsym(libc::RTLD_NEXT, read_name.as_ptr());
        std::mem::transmute(read_ptr)
    };
    unsafe {
        match SECRET_DESCRIPTOR_TO_POSITION.lock().unwrap().get_mut(&fd) {
            Some(position) => {
                if *position >= SECRET_CONTENT.len() {
                    return 0;
                }
                let bytes_to_read = std::cmp::min(count, SECRET_CONTENT.len() - *position);
                let _ = libc::memcpy(
                    buf,
                    SECRET_CONTENT.as_ptr().offset(*position as isize) as *mut libc::c_void,
                    bytes_to_read,
                );
                *position += bytes_to_read as usize;
                return bytes_to_read as libc::ssize_t;
            }
            None => {
                return original_read(fd, buf, count);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::CString, fs::File, io::Read, os::fd::FromRawFd};

    fn assert_file_content(
        filename: &str,
        open_mode: libc::c_int,
        expected_content: &str,
        is_secret: bool,
    ) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let file_path = std::path::PathBuf::from(manifest_dir)
            .join("resources")
            .join("test")
            .join(filename);
        let path = CString::new(file_path.into_os_string().into_string().unwrap()).unwrap();
        let fd: i32 = open(path.as_ptr(), open_mode);
        assert!(fd >= 0, "Failed to open file");
        if is_secret {
            assert!(
                unsafe {
                    SECRET_DESCRIPTOR_TO_POSITION
                        .lock()
                        .unwrap()
                        .get(&fd)
                        .is_some()
                },
                "Expected fd to be in the dictionary"
            );
        } else {
            assert!(
                unsafe {
                    SECRET_DESCRIPTOR_TO_POSITION
                        .lock()
                        .unwrap()
                        .get(&fd)
                        .is_none()
                },
                "Expected fd not be in the dictionary"
            );
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        assert_eq!(contents.trim(), expected_content);
        drop(file);
        close(fd);
    }

    #[test]
    fn open_normal_file() {
        assert_file_content("test.txt", libc::O_RDONLY, "I'm a test file", false);
    }

    #[test]
    fn open_secret_file() {
        assert_file_content("secret.txt", libc::O_RDONLY, SECRET_CONTENT, true);
    }

    #[test]
    fn open_secret_file_in_write_mode() {
        assert_file_content("secret.txt", libc::O_RDWR, "Nothing to see here.", false);
    }

    #[test]
    fn open_non_existent_file() {
        let path = CString::new("/non/existent/secret.txt").unwrap();
        let fd = open(path.as_ptr(), libc::O_RDONLY);
        assert!(fd < 0, "Expected open to fail");
    }

    #[test]
    fn close_secret_file() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let file_path = std::path::PathBuf::from(manifest_dir)
            .join("resources")
            .join("test")
            .join("secret.txt");
        let path = CString::new(file_path.into_os_string().into_string().unwrap()).unwrap();
        let fd = open(path.as_ptr(), libc::O_RDONLY);
        assert!(fd > 0, "Expected open to success");
        assert!(
            unsafe {
                SECRET_DESCRIPTOR_TO_POSITION
                    .lock()
                    .unwrap()
                    .get(&fd)
                    .is_some()
            },
            "Expected fd to be in the dictionary"
        );
        let result = close(fd);
        assert_eq!(result, 0, "Expected close to return 0");
        assert!(
            unsafe {
                SECRET_DESCRIPTOR_TO_POSITION
                    .lock()
                    .unwrap()
                    .get(&fd)
                    .is_none()
            },
            "Expected fd to be removed from the dictionary"
        );
    }
}
