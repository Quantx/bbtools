use std::fs::create_dir;
use std::path::PathBuf;
use std::env;

use crate::sbtools::*;
use xbe::XBE;
use bin::BIN;
use xpr::XPR;
use xpr::XPRFormat;
use xbo::XBO;
use lmt::LMT;
use ppd::PPD;
use xact::XSB;

pub mod sbtools;

const SB_TITLE_ID: u32 = 0x43430002;
fn sb_unpack(_xbe: &mut XBE, _game_path: &mut PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("Unpacking SB");
    
    return Ok(())
}

const LOC_TITLE_ID: u32 = 0x43430009;
const LOC_FPS: f32 = 20.0;
const LOC_XBO_SCALE: f32 = 0.01;

fn loc_unpack(_xbe: &mut XBE, game_path: &mut PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("Converting LOC");
    
    game_path.push("media");
    game_path.push("bin");
    
    /*** CONVERT TEXTURE FILES ***/
    game_path.push("TEXTURE");
    
    let _ = create_dir(&game_path);
    
    game_path.set_extension("bin");
    
    let mut bin_texture = BIN::open(&game_path)?;
    println!("Unpacking {} files from TEXTURE.bin", bin_texture.file_count());
    
    game_path.set_extension("");
    game_path.push("_");
    
    for file_index in 0..bin_texture.file_count() {
        let file_bytes = bin_texture.file_as_bytes(file_index)?;
        let mut xpr = match XPR::import_xpr(&file_bytes[..]) {
            Err(e) => {
                eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                continue;
            },
            Ok(v) => v,
        };
        
        println!("Converting file {:04} XPR {} to DDS", file_index, xpr.to_string());
        
        match xpr.format {
            XPRFormat::ARGB => {xpr.convert_argb_to_argb_lin()?;},
            XPRFormat::DXT1 => {xpr.convert_dxt1_to_dxt3()?;},
            _ => {},
        }
        
        let dds_name = format!("{:04}.dds", file_index);
        game_path.set_file_name(dds_name);
        
        xpr.write_to_dds(&game_path)?;
    }
    
    game_path.pop();
    game_path.pop();
    
    /*** CONVERT ANIMATION FILES ***/
    game_path.push("MOTION");
    
    let _ = create_dir(&game_path);
    
    game_path.set_extension("bin");
    
    let mut bin_motion = BIN::open(&game_path)?;
    println!("Unpacking {} files from MOTION.bin", bin_motion.file_count());
    
    game_path.set_extension("");
    game_path.push("_");
    
    // LMT and XBO files are bundled together into a GLTF 
    let mut lmts: Vec<LMT> = Vec::with_capacity(bin_motion.file_count());
    for file_index in 0..bin_motion.file_count() {
        let file_bytes = bin_motion.file_as_bytes(file_index)?;
        let mut lmt = match LMT::import(&file_bytes[..], LOC_XBO_SCALE, LOC_FPS) {
            Err(e) => {eprintln!("Failed to convert file {:04}, got error: {}", file_index, e); continue;},
            Ok(v) => v,
        };
        
        println!("Converting file {:04} LMT to GLTF", file_index);
        print!("{}", lmt);
        
        // Export GLBIN
        let glbin_name = format!("{:04}.glbin", file_index);
        game_path.set_file_name(&glbin_name);
        
        lmt.write_to_glbin(&game_path, true)?;
        
        // Export GLTF
        let gltf_name = format!("{:04}.gltf", file_index);
        game_path.set_file_name(gltf_name);

        lmt.write_to_gltf(&game_path)?;
        
        lmts.push(lmt);
    }
    
    game_path.pop();
    game_path.pop();
    
    /*** CONVERT MODEL FILES ***/
    game_path.push("MODEL");
    
    let _ = create_dir(&game_path);
    
    game_path.set_extension("bin");
    
    let mut bin_model = BIN::open(&game_path)?;
    println!("Unpacking {} files from MODEL.bin", bin_model.file_count());
    
    game_path.set_extension("");
    game_path.push("_");
    
    for file_index in 0..bin_model.file_count() {
        let file_bytes = bin_model.file_as_bytes(file_index)?;
        let mut xbo = match XBO::import_scaled(&file_bytes[..], LOC_XBO_SCALE) {
            Err(e) => {eprintln!("Failed to convert file {:04}, got error: {}", file_index, e); continue;},
            Ok(v) => v,
        };
        
        println!("Converting file {:04} XBO to GLTF", file_index);
        print!("{}", xbo);
        if file_index == 0 {
            // TODO: Compute this dynamically?
            xbo.set_animation(&lmts[20])?;
        }
        
        // Export GLBIN
        let glbin_name = format!("{:04}.glbin", file_index);
        game_path.set_file_name(&glbin_name);
        
        xbo.write_to_glbin(&game_path)?;
        
        // Export GLTF
        let gltf_name = format!("{:04}.gltf", file_index);
        game_path.set_file_name(gltf_name);

        xbo.write_to_gltf(&game_path)?;
    }
    
    game_path.pop();
    game_path.pop();
    
    /*** CONVERT HITBOX FILES ***/
    game_path.push("ATARI");
    
    let _ = create_dir(&game_path);
    
    game_path.set_extension("bin");
    
    let mut bin_atari = BIN::open(&game_path)?;
    println!("Unpacking {} files from ATARI.bin", bin_atari.file_count());
    
    game_path.set_extension("");
    game_path.push("_");
    
    for file_index in 0..bin_atari.file_count() {
        let file_bytes = bin_atari.file_as_bytes(file_index)?;
        let mut ppd = match PPD::import_scaled(&file_bytes, LOC_XBO_SCALE, None) {
            Err(e) => {eprintln!("Failed to convert file {:04}, got error: {}", file_index, e); continue;},
            Ok(v) => v,
        };
        
        println!("Converting file {:04} PPD to HBX", file_index);
        print!("{}", ppd);
        
        // Export HBX
        let gltf_name = format!("{:04}.hbx", file_index);
        game_path.set_file_name(gltf_name);
        
        ppd.write_to_hbx(&game_path)?;
    }
    
    game_path.pop();
    game_path.pop();


    game_path.pop(); // Exit bin/ directory to media/

    /*** CONVERT AUDIO FILES ***/
    game_path.push("sndeff");
    game_path.push("Bank.xsb");
    let xsb = XSB::open(&game_path)?;

    game_path.pop();
    xsb.export_banks(&game_path)?;

    return Ok(());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args();
    if args.len() < 2 {
        println!("Usage: ./sbtools <game_directory_path>");
        return Ok(());
    }
    
    let game_path_str = args.last().expect("Missing path to game directory");
    let mut game_path = PathBuf::from(game_path_str);
    
    // Append the executable file to the path if not present
    if game_path.is_dir() {
        game_path.push("default.xbe");
    }
    
    if !game_path.is_file() {
        println!("Coult not locate game XBE at: {}", game_path.display());
        return Ok(());
    }

    let mut xbe = XBE::open(&game_path)?;
    
    game_path.pop(); // Adjust the path to point at the root directory containing the game
    
    match xbe.title_id {
        SB_TITLE_ID  =>  sb_unpack(&mut xbe, &mut game_path)?,
        LOC_TITLE_ID => loc_unpack(&mut xbe, &mut game_path)?,
        _ => println!("Unknown title ID {:08X}", xbe.title_id),
    }
    
    return Ok(());
}
