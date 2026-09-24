mod common;

use common::assets_root;
use gta_sim::config::{
    load_config,
    manifest::{AssetLicense, THIRD_PARTY_DIR, THIRD_PARTY_MANIFEST, ThirdPartyManifest},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/manifest")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("GATE BROKEN: fixture {name} missing"))
}

fn shipped() -> ThirdPartyManifest {
    load_config::<ThirdPartyManifest>(&assets_root(), THIRD_PARTY_MANIFEST)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
}

#[test]
fn manifest_fixtures_are_judged() {
    for name in [
        "valid_minimal.ron",
        "valid_rig.ron",
        "valid_rig_none.ron",
        "valid_github_ofl.ron",
    ] {
        let valid = ron::from_str::<ThirdPartyManifest>(&fixture(name))
            .unwrap_or_else(|e| panic!("{name} must parse: {e}"));
        valid
            .validate()
            .unwrap_or_else(|e| panic!("{name} must validate: {e}"));
    }

    for name in [
        "bad_unknown_field.ron",
        "bad_duplicate_field.ron",
        "bad_license.ron",
    ] {
        assert!(
            ron::from_str::<ThirdPartyManifest>(&fixture(name)).is_err(),
            "{name} must be rejected by the parser"
        );
    }
    for (name, keyword) in [
        ("bad_absolute_path.ron", "path"),
        ("bad_parent_path.ron", "path"),
        ("bad_duplicate_path.ron", "duplicate"),
        ("bad_hash.ron", "sha256"),
        ("bad_license_file.ron", "license_file"),
        ("bad_url.ron", "url"),
        ("bad_rig_model.ron", "rig"),
        ("bad_rig_duplicate_clip.ron", "rig"),
        ("bad_github_url.ron", "url"),
        // "page" alone would match the "/media/pages/" of a Kenney url in any url error.
        ("bad_license_source.ron", "OFL requires page"),
        ("bad_ofl_version.ron", "url"),
    ] {
        let manifest = ron::from_str::<ThirdPartyManifest>(&fixture(name))
            .unwrap_or_else(|e| panic!("{name} must parse (its defect is semantic): {e}"));
        let error = manifest
            .validate()
            .expect_err(&format!("{name} must fail validate"));
        assert!(
            error.contains(keyword),
            "{name}: error {error:?} does not mention {keyword:?}"
        );
    }
}

#[test]
fn shipped_manifest_is_valid() {
    let manifest = shipped();
    manifest.validate().unwrap_or_else(|e| panic!("{e}"));
    let names = manifest
        .packs
        .iter()
        .map(|p| p.name.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        names,
        BTreeSet::from([
            "city-kit-roads",
            "city-kit-suburban",
            "city-kit-industrial",
            "mini-characters",
            "inter",
            "impact-sounds",
            "interface-sounds",
            "music-jingles",
            "car-kit"
        ])
    );
    let audio = ["impact-sounds", "interface-sounds", "music-jingles"];
    for pack in manifest.packs.iter().filter(|p| {
        p.name != "mini-characters"
            && p.name != "inter"
            && p.name != "car-kit"
            && !audio.contains(&p.name.as_str())
    }) {
        assert_eq!(pack.files.len(), 4, "pack {} file count", pack.name);
        assert!(pack.rig.is_none(), "pack {} has no rig", pack.name);
    }
    // 30 impact .ogg + License; click_001, toggle_001 + License; jingles_HIT00, jingles_SAX01 + License;
    // sedan.glb, its colormap + License.
    for (name, files) in [
        ("impact-sounds", 31),
        ("interface-sounds", 3),
        ("music-jingles", 3),
        ("car-kit", 3),
    ] {
        let pack = manifest.packs.iter().find(|p| p.name == name).unwrap();
        assert_eq!(pack.files.len(), files, "{name} file count");
        assert!(pack.rig.is_none(), "{name} has no rig");
        assert_eq!(pack.license, AssetLicense::CC0, "{name} license");
    }
    let inter = manifest.packs.iter().find(|p| p.name == "inter").unwrap();
    assert_eq!(inter.files.len(), 3, "inter file count");
    assert_eq!(inter.license, AssetLicense::OFL);
    assert!(inter.rig.is_none());
    let characters = manifest
        .packs
        .iter()
        .find(|p| p.name == "mini-characters")
        .unwrap();
    assert_eq!(characters.files.len(), 14, "mini-characters file count");
    // Counts and clip order come from the byte audit of the pinned archive (TASK-005 scratch).
    let rig = characters
        .rig
        .as_ref()
        .expect("mini-characters records its rig");
    assert_eq!(rig.models.len(), 12);
    assert_eq!(rig.skinned_meshes.len(), 2);
    assert_eq!(rig.joints.len(), 7);
    assert_eq!(rig.clips.len(), 32);
    for (clip, index) in [
        ("idle", 1),
        ("walk", 2),
        ("sprint", 3),
        ("jump", 4),
        ("fall", 5),
    ] {
        assert_eq!(rig.clip_index(clip), Some(index), "clip {clip}");
    }
    assert_eq!(rig.clip_index("run"), None);
}

fn files_under(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in fs::read_dir(&at).unwrap_or_else(|e| panic!("read_dir {}: {e}", at.display()))
        {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path.strip_prefix(dir).unwrap();
            let parts = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            out.insert(parts.join("/"));
        }
    }
    out
}

#[test]
fn local_assets_match_manifest() {
    let manifest = shipped();
    let root = assets_root().path(THIRD_PARTY_DIR);
    let pack_dir = |name: &str| -> PathBuf { root.join(name) };
    let (present, absent): (Vec<_>, Vec<_>) = manifest
        .packs
        .iter()
        .partition(|p| pack_dir(&p.name).is_dir());
    if present.is_empty() {
        eprintln!(
            "SKIP file check: third-party packs not fetched; run python tools/fetch_assets.py"
        );
        return;
    }
    assert!(
        absent.is_empty(),
        "partial install: packs {:?} are missing; run python tools/fetch_assets.py",
        absent.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
    for pack in &manifest.packs {
        let dir = pack_dir(&pack.name);
        let expected = pack
            .files
            .iter()
            .map(|f| f.path.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            files_under(&dir),
            expected,
            "pack {}: files on disk differ from the manifest",
            pack.name
        );
        for file in &pack.files {
            let bytes = fs::read(dir.join(&file.path)).unwrap();
            let actual = format!("{:x}", Sha256::digest(&bytes));
            assert_eq!(
                actual, file.sha256,
                "pack {}: {} sha256 {actual}, manifest {}",
                pack.name, file.path, file.sha256
            );
        }
        let license = fs::read(dir.join(&pack.license_file)).unwrap();
        for &needle in pack.license.markers() {
            assert!(
                license.windows(needle.len()).any(|w| w == needle),
                "pack {}: {} does not state {:?}",
                pack.name,
                pack.license_file,
                String::from_utf8_lossy(needle)
            );
        }
    }
    let packs = manifest
        .packs
        .iter()
        .map(|p| p.name.as_str())
        .collect::<BTreeSet<_>>();
    for entry in fs::read_dir(&root).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        let known =
            name == "manifest.ron" || (entry.path().is_dir() && packs.contains(name.as_str()));
        assert!(known, "unexpected entry {name} under {}", root.display());
    }
}
