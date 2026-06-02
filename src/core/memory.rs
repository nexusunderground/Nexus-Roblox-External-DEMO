#![allow(dead_code)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(target_os = "windows")]
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
#[cfg(target_os = "windows")]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32First, Module32Next, Process32First, Process32Next,
    MODULEENTRY32, PROCESSENTRY32, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};

pub const MAX_USER_ADDRESS: u64 = 0x7FFFFFFFFFFF;
const SSO_THRESHOLD: i32 = 16;
const MAX_STRING_LEN: i32 = 512;
static USE_SYSCALLS: AtomicBool = AtomicBool::new(false);


pub fn enable_syscalls() {
    USE_SYSCALLS.store(true, Ordering::SeqCst);
    tracing::info!("Syscall mode enabled");
}

#[inline]
pub fn is_syscall_mode() -> bool {
    USE_SYSCALLS.load(Ordering::Relaxed)
}

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Process not found: {0}")]
    ProcessNotFound(String),

    #[error("Module not found: {0}")]
    ModuleNotFound(String),

    #[error("Failed to open process: {0}")]
    OpenProcessFailed(String),

    #[error("Invalid address: 0x{0:x}")]
    InvalidAddress(u64),

    #[error("Read failed at 0x{0:x}")]
    ReadFailed(u64),

    #[error("Write failed at 0x{0:x}")]
    WriteFailed(u64),
}

#[inline]
pub fn is_valid_address(addr: u64) -> bool {
    addr != 0 && addr < MAX_USER_ADDRESS
}

pub struct Memory {
    process_id: u32,
    #[cfg(target_os = "windows")]
    process_handle: HANDLE,
    base_address: u64,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

impl Memory {
    pub fn new() -> Self {
        Self {
            process_id: 0,
            #[cfg(target_os = "windows")]
            process_handle: HANDLE(std::ptr::null_mut()),
            base_address: 0,
        }
    }

    pub fn attach(&mut self, process_name: &str) -> Result<(), MemoryError> {
        self.find_process_id(process_name)?;
        self.open_process()?;
        self.find_module_address(process_name)?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn find_process_id(&mut self, process_name: &str) -> Result<(), MemoryError> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(|e| MemoryError::ProcessNotFound(e.to_string()))?;

            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };

            if Process32First(snapshot, &mut entry).is_ok() {
                loop {
                    let exe_name = Self::extract_string(&entry.szExeFile);

                    if exe_name.eq_ignore_ascii_case(process_name) {
                        self.process_id = entry.th32ProcessID;
                        let _ = CloseHandle(snapshot);
                        tracing::debug!("Found process {} with PID {}", process_name, self.process_id);
                        return Ok(());
                    }

                    if Process32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
            Err(MemoryError::ProcessNotFound(process_name.to_string()))
        }
    }

    #[cfg(target_os = "windows")]
    fn open_process(&mut self) -> Result<(), MemoryError> {
        unsafe {
            self.process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, self.process_id)
                .map_err(|e| MemoryError::OpenProcessFailed(e.to_string()))?;
            tracing::debug!("Opened process handle: {:?}", self.process_handle);
            Ok(())
        }
    }


    #[cfg(target_os = "windows")]
    fn find_module_address(&mut self, module_name: &str) -> Result<(), MemoryError> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(
                TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                self.process_id,
            )
            .map_err(|e| MemoryError::ModuleNotFound(e.to_string()))?;

            let mut entry = MODULEENTRY32 {
                dwSize: std::mem::size_of::<MODULEENTRY32>() as u32,
                ..Default::default()
            };

            if Module32First(snapshot, &mut entry).is_ok() {
                loop {
                    let mod_name = Self::extract_string(&entry.szModule);

                    if mod_name.eq_ignore_ascii_case(module_name) {
                        self.base_address = entry.modBaseAddr as u64;
                        let _ = CloseHandle(snapshot);
                        tracing::debug!("Found module {} at 0x{:x}", module_name, self.base_address);
                        return Ok(());
                    }

                    if Module32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
            Err(MemoryError::ModuleNotFound(module_name.to_string()))
        }
    }

    #[cfg(target_os = "windows")]
    fn extract_string(bytes: &[i8]) -> String {
        let bytes: &[u8] =
            unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u8, bytes.len()) };
        let null_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..null_pos]).to_string()
    }

    #[inline]
    #[cfg(target_os = "windows")]
    pub fn read<T: Copy>(&self, address: u64) -> T {
        unsafe {
            let mut buffer: T = std::mem::zeroed();
            
            if USE_SYSCALLS.load(Ordering::Relaxed) {
                let mut bytes_read = 0usize;
                syscalls::nt_read_virtual_memory(
                    self.process_handle,
                    address as *const u8,
                    &mut buffer as *mut T as *mut u8,
                    std::mem::size_of::<T>(),
                    &mut bytes_read,
                );
            } else {
                let mut bytes_read = 0usize;
                let _ = ReadProcessMemory(
                    self.process_handle,
                    address as *const c_void,
                    &mut buffer as *mut T as *mut c_void,
                    std::mem::size_of::<T>(),
                    Some(&mut bytes_read),
                );
            }

            buffer
        }
    }

    #[inline]
    #[cfg(target_os = "windows")]
    pub fn read_f32_pair(&self, address: u64) -> (f32, f32) {
        let data: [f32; 2] = self.read(address);
        (data[0], data[1])
    }

    #[inline]
    #[cfg(target_os = "windows")]
    pub fn read_bytes(&self, address: u64, buffer: &mut [u8]) -> usize {
        unsafe {
            let mut bytes_read = 0usize;
            
            if USE_SYSCALLS.load(Ordering::Relaxed) {
                syscalls::nt_read_virtual_memory(
                    self.process_handle,
                    address as *const u8,
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    &mut bytes_read,
                );
            } else {
                let _ = ReadProcessMemory(
                    self.process_handle,
                    address as *const c_void,
                    buffer.as_mut_ptr() as *mut c_void,
                    buffer.len(),
                    Some(&mut bytes_read),
                );
            }
            
            bytes_read
        }
    }

    #[cfg(target_os = "windows")]
    pub fn read_checked<T: Copy>(&self, address: u64) -> Result<T, MemoryError> {
        if !is_valid_address(address) {
            return Err(MemoryError::InvalidAddress(address));
        }

        unsafe {
            let mut buffer: T = std::mem::zeroed();
            let mut bytes_read = 0usize;

            ReadProcessMemory(
                self.process_handle,
                address as *const c_void,
                &mut buffer as *mut T as *mut c_void,
                std::mem::size_of::<T>(),
                Some(&mut bytes_read),
            )
            .map_err(|_| MemoryError::ReadFailed(address))?;

            if bytes_read != std::mem::size_of::<T>() {
                return Err(MemoryError::ReadFailed(address));
            }

            Ok(buffer)
        }
    }

    #[cfg(target_os = "windows")]
    pub fn read_string(&self, address: u64) -> String {
        let length = self.read::<i32>(address + 0x10);

        if length <= 0 || length > MAX_STRING_LEN {
            return String::new();
        }

        let data_addr = if length >= SSO_THRESHOLD {
            self.read::<u64>(address)
        } else {
            address
        };

        if !is_valid_address(data_addr) {
            return String::new();
        }

        let mut buffer = vec![0u8; length as usize];

        unsafe {
            let mut bytes_read = 0usize;
            if ReadProcessMemory(
                self.process_handle,
                data_addr as *const c_void,
                buffer.as_mut_ptr() as *mut c_void,
                length as usize,
                Some(&mut bytes_read),
            )
            .is_err()
                || bytes_read == 0
            {
                return String::new();
            }
        }

        String::from_utf8_lossy(&buffer).to_string()
    }

    #[cfg(not(target_os = "windows"))]
    pub fn read_string(&self, _address: u64) -> String {
        String::new()
    }

    #[inline]
    #[cfg(target_os = "windows")]
    pub fn write<T: Copy>(&self, address: u64, value: T) {
        unsafe {
            if USE_SYSCALLS.load(Ordering::Relaxed) {
                let mut bytes_written = 0usize;
                syscalls::nt_write_virtual_memory(
                    self.process_handle,
                    address as *mut u8,
                    &value as *const T as *const u8,
                    std::mem::size_of::<T>(),
                    &mut bytes_written,
                );
            } else {
                WriteProcessMemory(
                    self.process_handle,
                    address as *const c_void,
                    &value as *const T as *const c_void,
                    std::mem::size_of::<T>(),
                    None,
                )
                .ok();
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub fn write_checked<T: Copy>(&self, address: u64, value: T) -> Result<(), MemoryError> {
        if !is_valid_address(address) {
            return Err(MemoryError::InvalidAddress(address));
        }

        unsafe {
            WriteProcessMemory(
                self.process_handle,
                address as *const c_void,
                &value as *const T as *const c_void,
                std::mem::size_of::<T>(),
                None,
            )
            .map_err(|_| MemoryError::WriteFailed(address))?;
        }

        Ok(())
    }

    #[inline]
    pub fn base_address(&self) -> u64 {
        self.base_address
    }

    #[inline]
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    #[cfg(target_os = "windows")]
    #[inline]
    pub fn handle(&self) -> HANDLE {
        self.process_handle
    }

    /// Resolve the Roblox camera address via fake_dm → dm → workspace → camera.
    pub fn resolve_camera_address(&self) -> Option<u64> {
        use crate::core::offsets::{fake_datamodel, datamodel, workspace};

        let fake_dm = self.read::<u64>(self.base_address + fake_datamodel::pointer());
        if !is_valid_address(fake_dm) { return None; }

        let dm = self.read::<u64>(fake_dm + fake_datamodel::real_datamodel());
        if !is_valid_address(dm) { return None; }

        let ws = self.read::<u64>(dm + datamodel::workspace());
        if !is_valid_address(ws) { return None; }

        let cam = self.read::<u64>(ws + workspace::current_camera());
        if is_valid_address(cam) { Some(cam) } else { None }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if !self.process_handle.0.is_null() {
            unsafe {
                let _ = CloseHandle(self.process_handle);
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub mod syscalls {
    use std::arch::asm;
    use windows::Win32::Foundation::HANDLE;

    mod ssn {
        pub const READ_VIRTUAL_MEMORY: u32 = 0x3F;
        pub const WRITE_VIRTUAL_MEMORY: u32 = 0x3A;
    }

    #[cfg(target_arch = "x86_64")]
    pub unsafe fn nt_read_virtual_memory(
        process_handle: HANDLE,
        base_address: *const u8,
        buffer: *mut u8,
        size: usize,
        bytes_read: *mut usize,
    ) -> i32 {
        let mut status: i32;

        asm!(
            "sub rsp, 0x28",
            "mov [rsp + 0x28], {bytes_read}",
            "mov r10, rcx",
            "mov eax, {ssn:e}",
            "syscall",
            "add rsp, 0x28",
            ssn = in(reg) ssn::READ_VIRTUAL_MEMORY,
            bytes_read = in(reg) bytes_read,
            in("rcx") process_handle.0,
            in("rdx") base_address,
            in("r8") buffer,
            in("r9") size,
            lateout("rax") status,
            out("r10") _,
            out("r11") _,
        );

        status
    }

    #[cfg(target_arch = "x86_64")]
    pub unsafe fn nt_write_virtual_memory(
        process_handle: HANDLE,
        base_address: *mut u8,
        buffer: *const u8,
        size: usize,
        bytes_written: *mut usize,
    ) -> i32 {
        let mut status: i32;

        asm!(
            "sub rsp, 0x28",
            "mov [rsp + 0x28], {bytes_written}",
            "mov r10, rcx",
            "mov eax, {ssn:e}",
            "syscall",
            "add rsp, 0x28",
            ssn = in(reg) ssn::WRITE_VIRTUAL_MEMORY,
            bytes_written = in(reg) bytes_written,
            in("rcx") process_handle.0,
            in("rdx") base_address,
            in("r8") buffer,
            in("r9") size,
            lateout("rax") status,
            out("r10") _,
            out("r11") _,
        );

        status
    }
}

