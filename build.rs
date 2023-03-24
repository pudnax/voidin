use anyhow::*;
use fs_extra::copy_items;
use fs_extra::dir::CopyOptions;
use std::env;
use std::path::Path;

// This script copies the `assets` folder to the app's `dist` folder
// Not sure if this is necessary (definitely not for WASM...)
fn main() -> Result<()> {
    // This tells cargo to rerun this script if something in /res/ changes.
    println!("cargo:rerun-if-changed=assets/*");

    let out_dir = env::var("OUT_DIR")?;
    let copy_options = CopyOptions::new().overwrite(true);
    copy_items(&[Path::new("assets/")], out_dir, &copy_options)?;

    Ok(())
}
