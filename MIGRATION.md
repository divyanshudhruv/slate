# Migrating from an older Slate

Slate 0.5 moved the install and the data folder out of `%USERPROFILE%\.burntfern\` and into `%LOCALAPPDATA%`. Migration happens automatically the first time a new build starts; there is nothing to run by hand.

## What moved

| Before | After |
| ------ | ----- |
| `%USERPROFILE%\.burntfern\slate\` | `%LOCALAPPDATA%\Burntfern\Slate\` |
| `%USERPROFILE%\.burntfern\slate\settings.json` | `%LOCALAPPDATA%\Burntfern\Slate\settings.json` |
| `%USERPROFILE%\.burntfern\slate\sets.json` | `%LOCALAPPDATA%\Burntfern\Slate\sets.json` |
| `%USERPROFILE%\.burntfern\slate\session.json` | `%LOCALAPPDATA%\Burntfern\Slate\session.json` |
| `%USERPROFILE%\.burntfern\slate\install.json` | `%LOCALAPPDATA%\Burntfern\Slate\install.json` |
| `%USERPROFILE%\.burntfern\config.json` (theme, accent) | folded into `settings.json` |
| `slate.exe` wherever it was downloaded | `%LOCALAPPDATA%\Programs\Burntfern\Slate\slate.exe` |

## What happens on the first launch

1. `%LOCALAPPDATA%\Burntfern\Slate\` and its `logs\` folder are created.
2. `settings.json`, `sets.json`, `session.json`, and `install.json` are copied from the old folder. If the destination already has a copy, the newer file wins and the file it replaces is kept in `backups\`.
3. Theme and accent are taken from the newest `config.json` (old root or old data folder) and written into `settings.json`.
4. Anything still missing is seeded from the built-in defaults.
5. `%USERPROFILE%\.burntfern\slate` is deleted once everything was copied, and `.burntfern` itself is removed only if it is now empty. Data belonging to other Burntfern apps is never touched.
6. If you ran a downloaded `slate-vX.Y.Z-x64.exe`, that copy installs itself to `%LOCALAPPDATA%\Programs\Burntfern\Slate\` and records the path in `install.json`. Later launches and updates use that copy.

## What is not migrated

- Logs, downloaded update packages, and backups from the old folder. They are removed with it once the copy succeeds.
- A previously downloaded update is not reused; Slate fetches the current release the next time it checks.

## Check that it worked

```powershell
Get-ChildItem "$env:LOCALAPPDATA\Burntfern\Slate"
Get-Content   "$env:LOCALAPPDATA\Burntfern\Slate\settings.json"
Test-Path     "$env:USERPROFILE\.burntfern\slate"   # expect False
```

- `settings.json` should still hold your theme, accent, and shortcuts.
- `install.json` should point at `%LOCALAPPDATA%\Programs\Burntfern\Slate\slate.exe`.
- If the old folder survives a normal quit, check `logs\` for `legacy config migration incomplete`; Slate retries on the next launch.

## If migration does not run

| Situation | What happens | Fix |
| --------- | ------------ | --- |
| Portable mode (`SLATE_PORTABLE=1` or `slate.portable` next to the exe) | No migration at all; settings live in `data\` beside the exe | Copy the four JSON files into `data\` yourself, or delete `slate.portable` to use the normal layout |
| Old folder locked or access denied | Nothing is copied or deleted; retried next launch | Close whatever holds the files, then start Slate again |
| A legacy file is not valid JSON | That file is skipped, the rest migrate, and the run is retried | Fix or delete the bad file, or let Slate seed defaults |
| The old folder still holds the only `slate.exe` | The folder is kept | Launch the new exe once and let it install itself, then start again |
| The running exe is inside the old folder | Cleanup is skipped | Run the installed copy from `%LOCALAPPDATA%\Programs\Burntfern\Slate\` |

## Manual migration

Use this only if the automatic pass cannot finish.

```powershell
# 1. Quit Slate from the tray, then:
$old = "$env:USERPROFILE\.burntfern\slate"
$new = "$env:LOCALAPPDATA\Burntfern\Slate"
New-Item -ItemType Directory -Force "$new\backups" | Out-Null
Copy-Item "$old\settings.json", "$old\sets.json", "$old\session.json", "$old\install.json" $new -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force $old
```

Leave `%USERPROFILE%\.burntfern\` in place if other Burntfern apps still use it.

## Rolling back

Older builds know nothing about `%LOCALAPPDATA%`, so they start with defaults in `%USERPROFILE%\.burntfern\slate`. To go back:

1. Quit Slate from the tray.
2. Download the older release and run it once.
3. Copy `settings.json` and `sets.json` from `%LOCALAPPDATA%\Burntfern\Slate\` into `%USERPROFILE%\.burntfern\slate\` while the old build is closed.

Updates themselves keep the previous binary as `slate.exe.old` next to the installed exe. If a swapped-in build never reaches its first frame, Slate rolls back to that copy automatically; `update-pending.json` records the swap until it is cleared.

## Portable mode

Portable builds never touch `%LOCALAPPDATA%` or `.burntfern`. Everything - settings, sets, session, updates, and logs - lives in `data\` beside the exe, so moving the folder moves the whole app. To make an installed copy portable, set `SLATE_PORTABLE=1` or drop an empty `slate.portable` file next to the exe, then copy the four JSON files into `data\`.
