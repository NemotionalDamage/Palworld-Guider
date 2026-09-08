//! Source-contract tests for the UE4SS adapter PowerShell scripts.
//!
//! These are string-level source contracts: they assert that each script
//! contains the safety behaviors required by the task brief without
//! executing the scripts (which would require the game, the save, or the
//! VS toolchain).

use std::fs;

use serde_json::Value;

const SCRIPTS_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");
const ADAPTER_DIRECTORY: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../adapter/read-only/ue4ss");
const UE4SS_DLL_SHA256: &str = "8AC18FBFFC1EF96B0662D4A2D537B3F224C26D65CAABA7989A9404C566102B26";

const ALL_SCRIPTS: [&str; 7] = [
    "Build-Ue4ssAdapter.ps1",
    "Test-InGameGuide.ps1",
    "Backup-PalworldSave.ps1",
    "Install-Ue4ssAdapter.ps1",
    "Uninstall-Ue4ssAdapter.ps1",
    "Setup-InGameGuide.ps1",
    "Start-InGameGuide.ps1",
];

fn script(name: &str) -> String {
    let path = format!("{SCRIPTS_DIRECTORY}/{name}");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn support_manifest_defines_the_reviewed_and_blocked_game_builds() {
    let path = format!("{ADAPTER_DIRECTORY}/support-manifest.json");
    let content =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"));
    let manifest: Value = serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("failed to parse {path}: {error}"));
    let object = manifest
        .as_object()
        .unwrap_or_else(|| panic!("support manifest must be a JSON object"));

    for key in [
        "schema_version",
        "game_build_ids",
        "game_version",
        "ue4ss_version",
        "ue4ss_dll_sha256",
        "member_variable_layout_sha256",
        "package_files",
        "blocked_game_build_ids",
    ] {
        assert!(
            object.contains_key(key),
            "support manifest must contain {key}"
        );
    }

    let supported_builds = manifest["game_build_ids"].as_array().unwrap();
    assert!(
        supported_builds.contains(&Value::String("24575825".to_owned())),
        "support manifest must allow reviewed Steam build 24575825"
    );
    let blocked_builds = manifest["blocked_game_build_ids"].as_array().unwrap();
    assert!(
        blocked_builds.contains(&Value::String("25094871".to_owned())),
        "support manifest must explicitly block Steam build 25094871"
    );
    assert_eq!(
        manifest["ue4ss_dll_sha256"], UE4SS_DLL_SHA256,
        "support manifest must pin the UE4SS.dll SHA256"
    );
    assert!(
        manifest["member_variable_layout_sha256"]
            .as_str()
            .is_some_and(|hash| hash.len() == 64
                && hash.chars().all(|character| character.is_ascii_hexdigit())),
        "support manifest must contain a hexadecimal member-variable layout SHA256"
    );
    assert!(
        manifest["package_files"]
            .as_array()
            .is_some_and(|files| !files.is_empty()),
        "support manifest must list package files"
    );
}

#[test]
fn preflight_discovers_and_validates_without_writing() {
    let content = script("Test-InGameGuide.ps1");
    for parameter in ["-GameRoot", "-Ue4ssDll", "-SaveDirectory"] {
        assert!(
            content.contains(parameter),
            "preflight must accept {parameter}"
        );
    }
    for marker in [
        "libraryfolders.vdf",
        "appmanifest_1623730.acf",
        "HKLM:\\SOFTWARE\\WOW6432Node\\Valve\\Steam",
        "HKLM:\\SOFTWARE\\Valve\\Steam",
        "InstallPath",
        "buildid",
        "$env:LOCALAPPDATA",
        "Level.sav",
        "LastWriteTime",
        "support-manifest.json",
        "Get-FileHash",
        "SHA256",
        "Mods",
        "Steam executable: ",
        "ConvertFrom-Json",
    ] {
        assert!(content.contains(marker), "preflight must contain {marker}");
    }
    for forbidden in [
        "Copy-Item",
        "Remove-Item",
        "Set-Content",
        "New-Item",
        "Move-Item",
    ] {
        assert!(
            !content.contains(forbidden),
            "read-only preflight must not contain {forbidden}"
        );
    }
    assert!(
        !content.contains("appmanifest_2379380.acf"),
        "preflight must use Palworld's Steam App ID 1623730"
    );
}

#[test]
fn preflight_requires_explicit_game_root_to_match_steam_manifest() {
    let content = script("Test-InGameGuide.ps1");
    let comparison_marker = ".Equals(";
    let mismatch_marker = "does not match the Palworld game directory from the Steam manifest";
    let mods_marker = "$ModsDirectory = Join-Path";

    let comparison_position = content
        .find(comparison_marker)
        .expect("preflight must compare explicit and manifest game roots");
    let mismatch_position = content
        .find(mismatch_marker)
        .expect("preflight must reject a game root that differs from the Steam manifest");
    let mods_position = content
        .find(mods_marker)
        .expect("preflight must locate the Mods directory");

    assert!(
        content.contains(
            "$ResolvedGameRoot.Equals($ResolvedManifestGameRoot, [System.StringComparison]::OrdinalIgnoreCase)",
        ),
        "preflight must compare the resolved explicit and manifest game roots case-insensitively"
    );
    assert!(
        comparison_position < mods_position && mismatch_position < mods_position,
        "preflight must reject a mismatched game root before checking Mods or UE4SS files"
    );
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
fn all_scripts_exist_and_are_non_empty() {
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
            !content.contains("api_key")
                && !content.contains("api-key")
                && (name == "Start-InGameGuide.ps1" || !content.contains("API_KEY")),
            "{name} references an API key (scripts must never read or print secrets)"
        );
    }
}
#[test]
fn build_script_pins_ue4ss_sha256_and_never_writes_to_the_game() {
    let content = script("Build-Ue4ssAdapter.ps1");
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
fn setup_composes_safe_stages_in_required_order() {
    let content = script("Setup-InGameGuide.ps1");
    for parameter in [
        "-GameRoot",
        "-Ue4ssDll",
        "-SaveDirectory",
        "-OutputDirectory",
    ] {
        assert!(content.contains(parameter), "setup must accept {parameter}");
    }

    let ordered_markers = [
        "$PreflightScript",
        "& $BackupScript",
        "& $BuildScript",
        "Verify-PackageManifest",
        "& $InstallScript",
    ];
    let mut previous_position = 0;
    for marker in ordered_markers {
        let position = content
            .find(marker)
            .unwrap_or_else(|| panic!("setup must contain ordered stage marker {marker}"));
        assert!(
            previous_position < position,
            "setup stage {marker} must run after the preceding required stage"
        );
        previous_position = position;
    }

    assert!(
        content.contains("SupportsShouldProcess"),
        "setup must support -WhatIf"
    );
    assert!(
        content.contains("$WhatIfPreference") && content.contains("What if:"),
        "setup must report its planned preflight, backup, build, verification, and install stages"
    );
    let what_if_position = content
        .find("$WhatIfPreference")
        .expect("setup must branch on -WhatIf");
    let install_position = content
        .find("& $InstallScript")
        .expect("setup must invoke the explicit installer");
    assert!(
        content
            .find("What if: installing")
            .is_some_and(|position| position < install_position),
        "setup -WhatIf must refuse to invoke the real game installer"
    );
    assert!(
        what_if_position < install_position,
        "setup must prevent the real installer under -WhatIf"
    );
    for marker in ["& $BackupScript", "& $BuildScript", "& $InstallScript"] {
        let position = content
            .find(marker)
            .unwrap_or_else(|| panic!("setup must contain stage marker {marker}"));
        assert!(
            what_if_position < position,
            "setup -WhatIf must return before invoking {marker}"
        );
    }

    assert!(
        content.contains(".local\\build") && content.contains(".local\\backups"),
        "setup defaults and writes must remain under repository .local"
    );
    assert!(
        content.contains("StartsWith") && content.contains(".local\\build"),
        "setup must reject an output directory outside repository .local\\build"
    );
    assert!(
        content.contains("Mods\\PalworldGuider already exists")
            && content.contains("Uninstall-Ue4ssAdapter.ps1"),
        "setup must refuse an existing package and direct users to uninstall first"
    );
}

#[test]
fn backup_and_install_verify_every_backed_up_file() {
    let backup = script("Backup-PalworldSave.ps1");
    assert!(
        backup.contains("Get-FileHash") && backup.contains("SHA256"),
        "backup must hash every copied file"
    );
    assert!(
        backup.contains("foreach ($CopiedFile in $CopiedFiles)") && backup.contains("sha256"),
        "backup manifest must include per-file records"
    );
    assert!(
        backup.contains("backup verification failed"),
        "backup must fail closed on a hash mismatch"
    );

    let install = script("Install-Ue4ssAdapter.ps1");
    let backup_verify_marker = "backup file hash mismatch";
    let target_marker = "$TargetDirectory = Join-Path";
    let backup_verify_position = install
        .find(backup_verify_marker)
        .expect("install must verify backup file hashes");
    let target_position = install
        .find(target_marker)
        .expect("install must derive the target package directory");
    assert!(
        backup_verify_position < target_position,
        "install must verify the complete backup before deriving or writing the game target"
    );
    assert!(
        install.contains("Copy-Item -LiteralPath $PackageManifestPath -Destination $InstalledManifestPath"),
        "install must include the package hash manifest in the installed package for startup validation"
    );
}

#[test]
fn start_validates_install_and_keeps_generated_token_in_memory() {
    let content = script("Start-InGameGuide.ps1");
    for parameter in [
        "$Port",
        "$AdapterPort",
        "$ServerExecutable",
        "$SteamExecutable",
    ] {
        assert!(content.contains(parameter), "start must accept {parameter}");
    }
    assert!(
        content.contains("$InstalledPackageDirectory = Join-Path"),
        "start must discover the installed Guider package"
    );
    assert!(
        content.contains("PalworldGuider.sha256")
            && content.contains("Get-FileHash")
            && content.contains("SHA256"),
        "start must validate every installed file against the package manifest"
    );
    assert!(
        content.contains("support-manifest.json")
            && content.contains("package_files")
            && content.contains("Count -ne $ExpectedPackageFileCount"),
        "start must validate the installed manifest against the reviewed package-file contract"
    );
    assert!(
        content.contains("[System.Guid]::NewGuid().ToString('N') * 2"),
        "start must generate exactly the specified 64-character random token"
    );
    assert!(
        content.contains(
            "$StartInfo.EnvironmentVariables['PALWORLD_GUIDER_GATEWAY_TOKEN'] = $GatewayToken",
        ),
        "start must pass the generated token only to the child process environment"
    );
    assert!(
        content.contains("$env:PALWORLD_GUIDER_GATEWAY_TOKEN = $GatewayToken")
            && content.contains("$env:PALWORLD_GUIDER_GATEWAY_PORT = [string]$AdapterPort"),
        "start must bind the generated token and gateway port to the launching session so Steam and Palworld inherit them"
    );
    assert!(
        !content.contains("SetEnvironmentVariable"),
        "start must never persist the token to the user or machine environment"
    );
    assert!(
        content.contains("--adapter-port") && content.contains("--adapter-token-env"),
        "start must launch guide-server in adapter mode"
    );
    assert!(
        content.contains("GUIDE_PROVIDER")
            && content.contains("GUIDE_MODEL")
            && content.contains("GUIDE_BASE_URL")
            && content.contains("GUIDE_DISABLE_REASON"),
        "start must pass provider configuration through from the current environment"
    );

    for line in content.lines() {
        if line.contains("$GatewayToken") {
            assert!(
                !line.contains("Write-Host")
                    && !line.contains("Write-Output")
                    && !line.contains("Set-Content")
                    && !line.contains("Out-File")
                    && !line.contains("Add-Content"),
                "start must not print or persist the generated token: {line}"
            );
        }
        if line.contains("OPENAI_API_KEY") || line.contains("$ProviderValue") {
            assert!(
                !line.contains("Write-Host")
                    && !line.contains("Write-Output")
                    && !line.contains("Set-Content")
                    && !line.contains("Out-File")
                    && !line.contains("Add-Content"),
                "start must not print or persist provider environment values: {line}"
            );
        }
    }
}

#[test]
fn start_refuses_a_running_game_and_launches_palworld_through_steam() {
    let content = script("Start-InGameGuide.ps1");
    assert!(
        content.contains("Get-Process -Name 'Palworld*'")
            && content.contains("Palworld is already running"),
        "start must refuse to run while Palworld is already running so the game can inherit the session token"
    );
    assert!(
        content.contains("Get-Process -Name 'steam'") && content.contains("CloseMainWindow"),
        "start must close a stale Steam client gracefully before relaunching it"
    );
    assert!(
        content.contains("did not close within 30 seconds"),
        "start must fail closed when Steam does not exit gracefully"
    );
    assert!(
        !content.contains("Stop-Process"),
        "start must never force-kill Steam or Palworld"
    );
    assert!(
        content
            .contains("Get-PreflightValue -Output $PreflightOutput -Prefix 'Steam executable: '"),
        "start must discover Steam through the preflight Steam executable value"
    );
    assert!(
        content.contains("Start-Process -FilePath $ResolvedSteamExecutable")
            && content.contains("-applaunch")
            && content.contains("1623730"),
        "start must launch Palworld app 1623730 through Steam so the game inherits the session environment"
    );
    assert!(
        content.contains("--game-version")
            && content.contains("[string]$SupportManifest.game_version"),
        "start must derive the game-version argument from the support manifest"
    );
    assert!(
        !content.contains("'1.0.3'"),
        "start must not hardcode the game version"
    );
}

#[test]
fn build_script_discovers_visual_studio_with_vswhere() {
    let content = script("Build-Ue4ssAdapter.ps1");
    assert!(
        content.contains("vswhere.exe")
            && content.contains("-latest")
            && content.contains("-products")
            && content.contains("-requires")
            && content.contains("Microsoft.VisualStudio.Component.VC.Tools.x86.x64"),
        "build must locate the Visual Studio C++ workload through vswhere"
    );
    assert!(
        !content.contains("Microsoft Visual Studio\\2022\\BuildTools"),
        "build must not assume the BuildTools installation path"
    );
    assert!(
        content.contains("Desktop development with C++"),
        "build failure must tell users which Visual Studio workload to install"
    );
}

#[test]
fn backup_script_refuses_unsafe_sources_destinations_and_running_game() {
    let content = script("Backup-PalworldSave.ps1");
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
    let content = script("Install-Ue4ssAdapter.ps1");
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
    assert_remove_item_targets_only_guider("Install-Ue4ssAdapter.ps1", &content);
}

#[test]
fn uninstall_script_removes_only_the_guider_package() {
    let content = script("Uninstall-Ue4ssAdapter.ps1");
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
    assert_remove_item_targets_only_guider("Uninstall-Ue4ssAdapter.ps1", &content);
}
