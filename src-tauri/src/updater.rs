//! Installs an update from inside the app. The release DMG is downloaded, the app in it
//! is checked the same way macOS checks it (signed by Parla's developer team, notarized
//! by Apple, the right bundle, a newer version), and a small script swaps it in once
//! Parla has quit, then opens the new version. Nothing unsigned is ever launched.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const TEAM_ID: &str = "5PNLAR99PK";
const BUNDLE_ID: &str = "com.bryanparreira.parla";

/// The Parla.app this process runs from, or None for a development build.
pub fn installed_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    // …/Parla.app/Contents/MacOS/parla
    let bundle = exe.parent()?.parent()?.parent()?.to_path_buf();
    (bundle.extension()? == "app").then_some(bundle)
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Couldn't run {program}: {e}"))?;
    // codesign reports details on stderr, even on success.
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(text)
    } else {
        Err(text.trim().to_string())
    }
}

/// Copies the app out of a downloaded DMG into `work`, then checks it.
pub fn extract(dmg: &Path, work: &Path) -> Result<PathBuf, String> {
    let mount = work.join("mount");
    fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    let mount_arg = mount.to_string_lossy().to_string();
    run(
        "hdiutil",
        &[
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-mountpoint",
            &mount_arg,
            &dmg.to_string_lossy(),
        ],
    )
    .map_err(|e| format!("Couldn't open the update: {e}"))?;

    let copied = (|| {
        let app = fs::read_dir(&mount)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "app"))
            .ok_or("The update doesn't contain Parla.")?;
        let target = work.join("Parla.app");
        run("ditto", &[&app.to_string_lossy(), &target.to_string_lossy()])?;
        Ok::<_, String>(target)
    })();
    let _ = run("hdiutil", &["detach", &mount_arg, "-force"]);
    copied
}

/// Refuses anything that isn't a genuine, notarized, newer Parla. Returns its version.
pub fn verify(app: &Path, current_version: &str) -> Result<String, String> {
    let path = app.to_string_lossy();
    run("codesign", &["--verify", "--deep", "--strict", &path])
        .map_err(|_| "The update's signature is broken, so it wasn't installed.".to_string())?;
    let details = run("codesign", &["-dv", "--verbose=2", &path])?;
    if !details.lines().any(|line| line.trim() == format!("TeamIdentifier={TEAM_ID}"))
        || !details.lines().any(|line| line.trim() == format!("Identifier={BUNDLE_ID}"))
    {
        return Err("The update isn't signed by Parla's developer, so it wasn't installed.".into());
    }
    // Gatekeeper's own verdict, which includes Apple's notarization.
    run("spctl", &["--assess", "--type", "execute", &path])
        .map_err(|_| "macOS doesn't trust the update, so it wasn't installed.".to_string())?;

    let plist = app.join("Contents/Info.plist");
    let version = run(
        "/usr/libexec/PlistBuddy",
        &["-c", "Print :CFBundleShortVersionString", &plist.to_string_lossy()],
    )?
    .trim()
    .to_string();
    if !crate::newer(&version, current_version) {
        return Err(format!("The download is Parla {version}, which isn't newer than this one."));
    }
    Ok(version)
}

/// Replaces `target` with `new_app` once process `pid` has quit, then opens it. The old
/// app is kept until the copy succeeds and put back if it doesn't.
pub fn schedule_swap(pid: u32, new_app: &Path, target: &Path, work: &Path) -> Result<(), String> {
    let parent = target.parent().ok_or("Parla isn't in a folder it can be updated in.")?;
    // The swap happens after Parla quits, when it can no longer report a problem, so
    // the folder has to be writable now.
    let probe = parent.join(".parla-update-check");
    fs::write(&probe, b"").map_err(|_| {
        format!(
            "Parla can't write to {}. Download the update and drag it into Applications instead.",
            parent.display()
        )
    })?;
    let _ = fs::remove_file(&probe);

    let backup = parent.join(".Parla-previous.app");
    let script = work.join("swap.sh");
    fs::write(&script, SWAP_SCRIPT).map_err(|e| e.to_string())?;
    Command::new("/bin/sh")
        .arg(&script)
        .arg(pid.to_string())
        .arg(new_app)
        .arg(target)
        .arg(&backup)
        .arg(work)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't start the update: {e}"))
}

const SWAP_SCRIPT: &str = r#"#!/bin/sh
# Arguments: pid, new app, installed app, backup path, work folder.
pid="$1"; new="$2"; target="$3"; backup="$4"; work="$5"
while kill -0 "$pid" 2>/dev/null; do sleep 0.2; done
rm -rf "$backup"
mv "$target" "$backup" || { open "$target"; exit 1; }
if ditto "$new" "$target"; then
  rm -rf "$backup"
else
  rm -rf "$target"
  mv "$backup" "$target"
fi
open "$target"
rm -rf "$work"
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // Needs the network: downloads the latest real release and runs every check on it.
    //   cargo test --lib updater -- --ignored --nocapture
    #[test]
    #[ignore]
    fn verifies_the_published_release() {
        let release = crate::engine::latest_release().expect("reach GitHub");
        let url = release.download.expect("release has a DMG");
        let work = std::env::temp_dir().join("parla-update-test");
        let _ = fs::remove_dir_all(&work);
        fs::create_dir_all(&work).unwrap();
        let dmg = work.join("Parla.dmg");
        let started = Instant::now();
        crate::engine::download(&url, &dmg).expect("download");
        println!("downloaded {} bytes in {:?}", fs::metadata(&dmg).unwrap().len(), started.elapsed());
        let app = extract(&dmg, &work).expect("extract");
        // Pretend to be older, so the published release counts as an update.
        println!("verified version {}", verify(&app, "0.0.1").expect("verify"));
        assert!(verify(&app, "999.0.0").is_err(), "an older download must be refused");
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn swaps_the_app_in_after_the_old_one_quits() {
        let work = std::env::temp_dir().join(format!("parla-swap-test-{}", std::process::id()));
        let apps = work.join("Applications");
        let target = apps.join("Parla.app");
        let new_app = work.join("new/Parla.app");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&new_app).unwrap();
        fs::write(target.join("version"), "old").unwrap();
        fs::write(new_app.join("version"), "new").unwrap();
        // The script opens the app at the end; a stand-in `open` keeps that out of the test.
        let bin = work.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("open"), "#!/bin/sh\necho opened > \"$1/opened\"\n").unwrap();
        run("chmod", &["+x", &bin.join("open").to_string_lossy()]).unwrap();

        let script = work.join("swap.sh");
        fs::write(&script, SWAP_SCRIPT).unwrap();
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
        let status = Command::new("/bin/sh")
            .env("PATH", path)
            .arg(&script)
            .arg("999999") // no such process: the swap starts right away
            .arg(&new_app)
            .arg(&target)
            .arg(apps.join(".Parla-previous.app"))
            .arg(work.join("scratch"))
            .status()
            .unwrap();
        assert!(status.success());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !target.join("opened").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(fs::read_to_string(target.join("version")).unwrap(), "new");
        assert!(target.join("opened").exists(), "the new version is opened");
        assert!(!apps.join(".Parla-previous.app").exists(), "the backup is cleaned up");
        let _ = fs::remove_dir_all(&work);
    }
}
