//! Start menu shortcut for the installed copy.

use std::path::{Path, PathBuf};

use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

/// Writes or refreshes the Start menu shortcut to the installed copy so search keeps finding it.
pub fn ensure_start_menu_shortcut(exe: &Path) -> Result<(), String> {
    let link_path = shortcut_path().ok_or_else(|| "APPDATA is not set".to_string())?;
    if let Some(dir) = link_path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("start menu dir: {err}"))?;
    }
    let target = HSTRING::from(exe.to_string_lossy().as_ref());
    let workdir = exe
        .parent()
        .map(|dir| HSTRING::from(dir.to_string_lossy().as_ref()));
    let link_file = HSTRING::from(link_path.to_string_lossy().as_ref());

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW =
            CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|err| err.to_string())?;
        link.SetPath(&target).map_err(|err| err.to_string())?;
        if let Some(dir) = workdir.as_ref() {
            let _ = link.SetWorkingDirectory(dir);
        }
        let _ = link.SetDescription(&HSTRING::from("Slate"));
        let _ = link.SetIconLocation(&target, 0);
        let file: IPersistFile = link.cast().map_err(|err| err.to_string())?;
        file.Save(&link_file, true).map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// True when the shortcut is already on disk, so older installs get one without a rewrite each launch.
pub fn start_menu_shortcut_exists() -> bool {
    shortcut_path().is_some_and(|path| path.exists())
}

fn shortcut_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\Slate.lnk"))
}
