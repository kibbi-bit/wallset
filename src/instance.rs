use std::io;

use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
        System::Threading::{CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, SetEvent},
    },
    core::w,
};

const MUTEX_NAME: windows::core::PCWSTR = w!("Local\\Wallset.SingleInstance");
const EVENT_NAME: windows::core::PCWSTR = w!("Local\\Wallset.Activate");

pub struct InstanceGuard {
    mutex: HANDLE,
    activation_event: HANDLE,
}

impl InstanceGuard {
    /// Returns `None` after notifying an already-running instance.
    pub fn acquire() -> io::Result<Option<Self>> {
        unsafe {
            let mutex = CreateMutexW(None, false, MUTEX_NAME).map_err(win_error)?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(mutex);
                if let Ok(event) = OpenEventW(EVENT_MODIFY_STATE, false, EVENT_NAME) {
                    let _ = SetEvent(event);
                    let _ = CloseHandle(event);
                }
                return Ok(None);
            }
            let activation_event =
                CreateEventW(None, false, false, EVENT_NAME).map_err(win_error)?;
            Ok(Some(Self {
                mutex,
                activation_event,
            }))
        }
    }

    pub fn activation_event(&self) -> HANDLE {
        self.activation_event
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.activation_event);
            let _ = CloseHandle(self.mutex);
        }
    }
}

fn win_error(error: windows::core::Error) -> io::Error {
    io::Error::other(error.to_string())
}
