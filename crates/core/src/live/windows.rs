//! Lecture de la mémoire d'un processus Windows, **en lecture seule**.
//!
//! Droits demandés : `PROCESS_QUERY_INFORMATION` (énumérer les zones avec `VirtualQueryEx`,
//! savoir si le processus vit encore) et `PROCESS_VM_READ` (`ReadProcessMemory`). Jamais
//! `PROCESS_VM_WRITE`, `PROCESS_VM_OPERATION` ni `PROCESS_CREATE_THREAD` : Kaleido ne peut
//! rien écrire dans l'émulateur, même par erreur.

use std::ffi::c_void;
use std::mem::{size_of, zeroed};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, STILL_ACTIVE};
use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD,
    PAGE_NOACCESS, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

use super::{LiveError, MemorySource, Region};

/// Processus en cours d'exécution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    /// Nom de l'exécutable (`melonDS.exe`).
    pub exe: String,
}

/// Liste des processus (instantané Toolhelp, sans console ni `tasklist`).
pub fn processes() -> Vec<ProcessInfo> {
    let mut out = Vec::new();
    // SAFETY: appels Win32 documentés ; la poignée est fermée avant de sortir.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut e: PROCESSENTRY32W = zeroed();
        e.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e) != 0;
        while ok {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            out.push(ProcessInfo { pid: e.th32ProcessID, exe: String::from_utf16_lossy(&e.szExeFile[..len]) });
            ok = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
    }
    out
}

/// Processus ouvert en lecture seule.
pub struct Process {
    handle: HANDLE,
    pid: u32,
}

// SAFETY: une poignée de processus peut être utilisée depuis n'importe quel thread.
unsafe impl Send for Process {}
unsafe impl Sync for Process {}

impl Process {
    pub fn open(pid: u32) -> Result<Self, LiveError> {
        // SAFETY: appel Win32 ; une poignée nulle signale l'échec.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
        if handle.is_null() {
            // SAFETY: lecture du code d'erreur du thread courant.
            let code = unsafe { GetLastError() };
            return Err(LiveError::Denied(format!("OpenProcess({pid}) : erreur {code}")));
        }
        Ok(Process { handle, pid })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: poignée obtenue par OpenProcess, fermée une seule fois.
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

fn readable(protect: u32) -> bool {
    if protect & (PAGE_GUARD | PAGE_NOACCESS) != 0 {
        return false;
    }
    let base = protect & 0xFF;
    [PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY].contains(&base)
}

impl MemorySource for Process {
    fn read(&self, addr: u64, buf: &mut [u8]) -> Result<(), LiveError> {
        let mut done = 0usize;
        // SAFETY: le tampon de destination est à nous et fait `buf.len()` octets ; la lecture
        // dans l'autre processus est vérifiée par le système (échec si une page est invalide).
        let ok = unsafe { ReadProcessMemory(self.handle, addr as *const c_void, buf.as_mut_ptr().cast(), buf.len(), &mut done) };
        if ok == 0 || done != buf.len() {
            return Err(if self.alive() { LiveError::Read { addr, len: buf.len() } } else { LiveError::Gone });
        }
        Ok(())
    }

    fn regions(&self) -> Vec<Region> {
        let mut out = Vec::new();
        let mut addr: u64 = 0;
        // Espace utilisateur Windows 64 bits : 128 Tio.
        while addr < 0x7FFF_FFFF_0000 {
            // SAFETY: structure de sortie à nous, taille passée explicitement.
            let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { zeroed() };
            let n = unsafe { VirtualQueryEx(self.handle, addr as *const c_void, &mut mbi, size_of::<MEMORY_BASIC_INFORMATION>()) };
            if n == 0 {
                break;
            }
            let base = mbi.BaseAddress as u64;
            let size = mbi.RegionSize as u64;
            if mbi.State == MEM_COMMIT && readable(mbi.Protect) {
                let allocation = mbi.AllocationBase as u64;
                // Zones contiguës de la même allocation : fusionnées (un tampon de RAM émulée
                // peut être découpé par des protections différentes).
                match out.last_mut() {
                    Some(Region { base: b, size: s, allocation: a }) if *a == allocation && *b + *s == base => *s += size,
                    _ => out.push(Region { base, size, allocation }),
                }
            }
            match base.checked_add(size) {
                Some(next) if next > addr => addr = next,
                _ => break,
            }
        }
        out
    }

    fn alive(&self) -> bool {
        let mut code = 0u32;
        // SAFETY: poignée valide tant que `self` existe.
        let ok = unsafe { GetExitCodeProcess(self.handle, &mut code) };
        ok != 0 && code == STILL_ACTIVE as u32
    }
}
