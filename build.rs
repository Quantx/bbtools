use std::env;
use std::fs::File;

use flate2::{Compression, write::GzEncoder};
use pathdiff::diff_paths;
use tar::Builder;

// Example custom build script.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=godot/project.godot");
    println!("cargo::rerun-if-changed=godot/addons/");
    println!("cargo::rerun-if-changed=godot/tests/");
    println!("cargo::rerun-if-changed=bin/");

    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS not specified");

    let profile = env::var("PROFILE").unwrap_or_default();

    let tar_path = format!("internal_{}_{}.tgz", profile, target_os);

    {
        let src_to_tar_path = diff_paths(&tar_path, "src/").unwrap();
        println!(
            "cargo::rustc-env=INTERNAL_TAR_PATH={}",
            src_to_tar_path.to_str().unwrap()
        );
    }

    // Don't actually compress debug builds for faster build times
    let compression = if profile == "debug" {
        Compression::none()
    } else {
        Compression::best()
    };

    let file = File::create(tar_path)?;
    let mut tar_builder = Builder::new(GzEncoder::new(file, compression));
    tar_builder.follow_symlinks(false);

    // Add specific OS executables
    tar_builder.append_dir_all("bin", format!("bin/{}", target_os))?;
    tar_builder.append_dir_all("bin", "bin/license")?;

    // Add godot root directory
    tar_builder.append_dir("godot/", "godot/")?;

    tar_builder.append_path("godot/project.godot")?;
    tar_builder.append_path("godot/export_presets.cfg")?;

    tar_builder.append_dir_all("godot/addons", "godot/addons")?;

    tar_builder.append_dir_all("godot/tests", "godot/tests")?;

    // Finish writing the Tar file and return the underlying writer
    tar_builder.into_inner()?;

    return Ok(());
}
