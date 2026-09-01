//! Source-contract tests for the G5 adapter PowerShell scripts.
//!
//! These are string-level source contracts: they assert that each script
//! contains the safety behaviors required by the task brief without
//! executing the scripts (which would require the game, the save, or the
//! VS toolchain).

use std::fs;

const SCRIPTS_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");
const UE4SS_DLL_SHA256: &str = "8AC18FBFFC1EF96B0662D4A2D537B3F224C26D65CAABA7989A9404C566102B26";

const ALL_SCRIPTS: [&str; 4] = [
    "Build-G5Ue4ss.ps1",
    "Backup-G5Save.ps1",
    "Install-G5Ue4ss.ps1",
    "Uninstall-G5Ue4ss.ps1",
];

fn script(name: &str) -> String {
    let path = format!("{SCRIPTS_DIRECTORY}/{name}");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

/// Every line that removes filesystem content must target only the Guider
/// package or the mods.txt enablement file (which Install may remove again
/// during rollback if it created it), never another mod, the Mods root, or
/// UE4SS files.
fn assert_remove_item_targets_only_guider(name: &str, content: &str) {
    for line in content.lines() {
        if line.contains("Remove-Item") {
            assert!(
                line.contains("PalworldGuider") || line.contains("ModsFile"),
                "{name} removes content on a line that does not target the PalworldGuider package or mods.txt: {line}"
            );
        }
    }
}

#[test]
fn all_four_scripts_exist_and_are_non_empty() {
    for name in ALL_SCRIPTS {
        let content = script(name);
        assert!(!content.trim().is_empty(), "{name} is empty");
    }
}

#[test]
fn no_script_reads_or_prints_a_token() {
    for name in ALL_SCRIPTS {
        let content = script(name);
        assert!(
            !content.contains("Read-Host"),
            "{name} reads input with Read-Host (no token or secret input is allowed)"
        );
        assert!(
            !content.contains("PALWORLD_GUIDER_GATEWAY_TOKEN"),
            "{name} references the gateway token (scripts must never read or print it)"
        );
        assert!(
            !content.contains("api_key")
                && !content.contains("api-key")
                && !content.contains("API_KEY"),
            "{name} references an API key (scripts must never read or print secrets)"
        );
    }
}
#[test]
fn build_script_pins_ue4ss_sha256_and_never_writes_to_the_game() {
    let content = script("Build-G5Ue4ss.ps1");
    assert!(
        content.contains(UE4SS_DLL_SHA256),
        "build must pin the exact UE4SS.dll SHA256"
    );
    assert!(
        content.contains("Get-FileHash") && content.contains("SHA256"),
        "build must compute SHA256 file hashes"
    );
    assert!(
        content.contains("-Ue4ssDll"),
        "build must accept the -Ue4ssDll parameter"
    );
    assert!(
        content.contains("-OutputDirectory"),
        "build must accept the -OutputDirectory parameter"
    );
    assert!(
        !content.contains("Steam\\steamapps"),
        "build must never reference a Steam game path"
    );
    assert!(
        !content.contains("Mods"),
        "build must never reference the game Mods directory"
    );
}

#[test]
fn backup_script_refuses_unsafe_sources_destinations_and_running_game() {
    let content = script("Backup-G5Save.ps1");
    assert!(
        content.contains("Level.sav"),
        "backup must look for Level.sav in the source"
    );
    assert!(
        content.contains("Level.sav not found"),
        "backup must refuse a source with no Level.sav"
    );
    assert!(
        content.contains("refusing to overwrite"),
        "backup must refuse to overwrite an existing non-empty destination"
    );
    assert!(
        content.contains("Resolve-Path") && content.contains(".local"),
        "backup must resolve paths for a .local containment check"
    );
    assert!(
        content.contains("StartsWith"),
        "backup must use a resolved-path containment check on .local"
    );
    assert!(
        content.contains("outside .local"),
        "backup must refuse destinations outside the repo .local directory"
    );
    assert!(
        content.contains("Get-Process") && content.contains("Palworld"),
        "backup must refuse while a Palworld process is running"
    );
    assert!(
        content.contains("-LiteralPath"),
        "backup must copy with -LiteralPath"
    );
    assert!(
        content.contains("Measure-Object")
            && content.contains("Count")
            && content.contains("Length"),
        "backup must verify file count and total byte length"
    );
    assert!(
        content.contains("retention") && content.contains("30"),
        "backup must record 30-day retention"
    );
}

#[test]
fn install_script_requires_verified_backup_before_any_mods_write() {
    let content = script("Install-G5Ue4ss.ps1");
    let backup_marker = "no verified backup";
    let mods_marker = "Mods\\PalworldGuider";
    let backup_position = content
        .find(backup_marker)
        .expect("install must check for a verified backup");
    let mods_position = content
        .find(mods_marker)
        .expect("install must target the Mods\\PalworldGuider folder");
    assert!(
        backup_position < mods_position,
        "install must verify the backup before any write to the game Mods directory"
    );
    assert!(
        content.contains("Get-Process") && content.contains("Palworld"),
        "install must require the game to be stopped"
    );
    assert!(
        content.contains("PalworldGuider.sha256"),
        "install must require the package hash manifest"
    );
    assert!(
        content.contains("Count -eq 0") && content.contains("no verifiable entries"),
        "install must fail closed when the package hash manifest has no verifiable entries"
    );
    assert!(
        content.contains("Get-FileHash") && content.contains("SHA256"),
        "install must verify staged files against the package hash manifest"
    );
    assert!(
        content.contains(mods_marker),
        "install must install exactly into Mods\\PalworldGuider"
    );
    assert!(
        content.contains("mods.txt"),
        "install must manage the mods.txt enablement file"
    );
    assert!(
        content.contains("PalworldGuider : 1"),
        "install must enable the PalworldGuider line in mods.txt"
    );
    assert!(
        content.contains("-notmatch"),
        "install must preserve mods.txt lines belonging to other mods"
    );
    assert!(
        content.contains("-LiteralPath"),
        "install must copy with -LiteralPath"
    );
    assert_remove_item_targets_only_guider("Install-G5Ue4ss.ps1", &content);
}

#[test]
fn uninstall_script_removes_only_the_guider_package() {
    let content = script("Uninstall-G5Ue4ss.ps1");
    assert!(
        content.contains("Get-Process") && content.contains("Palworld"),
        "uninstall must require the game to be stopped"
    );
    assert!(
        content.contains("PalworldGuider"),
        "uninstall must reference the PalworldGuider package"
    );
    assert!(
        content.contains("mods.txt"),
        "uninstall must remove the mods.txt entry"
    );
    assert!(
        content.contains("-notmatch"),
        "uninstall must preserve other mods.txt entries"
    );
    assert!(
        content.contains("-LiteralPath"),
        "uninstall must use -LiteralPath"
    );
    assert!(
        content.contains("Removed:"),
        "uninstall must report what it removed"
    );
    assert!(
        !content.contains("UE4SS.dll"),
        "uninstall must never touch UE4SS files"
    );
    assert_remove_item_targets_only_guider("Uninstall-G5Ue4ss.ps1", &content);
}
