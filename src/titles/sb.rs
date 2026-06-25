use std::fs::File;
use std::fs::create_dir;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::bbtools::obj::WeaponModelConfig;
use crate::bbtools::stg::Mission;
use crate::bbtools::*;
use bin::BIN;
use byteorder::LittleEndian;
use byteorder::ReadBytesExt;
use lmt::LMT;
use obj::{ModelConfigDB, OBJ};
use ppd::PPD;
use text::import_cc_text;
use xbe::XBE;
use xbo::XBO;
use xpr::{XPR, XPRFormat};

pub const TITLE_PREFIX: &str = "sb";
pub const TITLE_ID: u32 = 0x43430002;
const GAME_FPS: f32 = 20.0;
const GAME_SCALE: f32 = 2.0;
const GAME_MISSION_COUNT: usize = 24;

const ADVANCE_BIN: &str = "ADVANCE.bin";
const ATARI_BIN: &str = "ATARI.bin";
const DVDMOVIE_BIN: &str = "DVDMOVIE.bin";
const LSQ_BIN: &str = "LSQ.bin";
const MODEL_BIN: &str = "MODEL.bin";
const MOTION_BIN: &str = "MOTION.bin";
const RTN_BIN: &str = "RTN.bin";
const SOUND_BIN: &str = "SOUND.bin";
const TEXMOVIE_BIN: &str = "TEXMOVIE.bin";
const TEXOBJ_BIN: &str = "TEXOBJ.bin";

const ADVANCE_TEXTURE_END: usize = 30;
const ADVANCE_MODEL_END: usize = 55;

// fn get_file_name_list(bin_name: &str) -> std::str::Lines<'static> {
//     match bin_name {
//         ATARI_BIN => include_str!("loc/bin_file_names/ATARI.txt"),
//         LSQ_BIN => include_str!("loc/bin_file_names/LSQ.txt"),
//         MODEL_BIN => include_str!("loc/bin_file_names/MODEL.txt"),
//         MOTION_BIN => include_str!("loc/bin_file_names/MOTION.txt"),
//         TEXTURE_BIN => include_str!("loc/bin_file_names/TEXTURE.txt"),
//         VTMODEL_BIN => include_str!("loc/bin_file_names/VTMODEL.txt"),
//         _ => unimplemented!("Unknown bin file"),
//     }
//     .lines()
// }

fn get_gltf_path(idx: usize) -> PathBuf {
    let mut path = PathBuf::from("/proprietary/sb/models/");
    path.push(format!("{:04}.gltf", idx));
    return path;
}

fn get_dds_object_path(idx: usize) -> PathBuf {
    let mut path = PathBuf::from("/proprietary/sb/textures/objects/");
    path.push(format!("{:04}.dds", idx));
    return path;
}

pub fn unpack(
    xbe: &mut XBE,
    game_path: &mut PathBuf,
    build_path: &mut PathBuf,
    godot_base_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Unpacking SB");

    build_path.push("sb");
    let _ = create_dir(&build_path);

    let build_title_path = build_path.clone();

    game_path.push("Media");

    /*** CONVERT GROUND TEXTURES FIRST ***/
    game_path.push("BIN");
    let mut ground_textures: Vec<XPR> = Vec::with_capacity(GAME_MISSION_COUNT);
    for mi in 0..GAME_MISSION_COUNT {
        game_path.push(format!("TEX{:02}.bin", mi));
        let mut bin_ground_texture = BIN::<File>::open(&game_path)?;
        game_path.pop();

        println!(
            "Unpacking {} files from TEX{:02}.bin",
            bin_ground_texture.file_count(),
            mi
        );

        let mut base_texture: Option<XPR> = None;

        for file_index in 0..bin_ground_texture.file_count() {
            let file_bytes = bin_ground_texture.file_as_bytes(file_index)?;
            let ground_texture = match XPR::import_xpr(&file_bytes) {
                Err(e) => {
                    eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                    None
                }
                Ok(mut xpr) => {
                    let tex_name = format!("{:04}", file_index);
                    println!(
                        "Converting file {:04}:{} XPR {} to DDS",
                        0,
                        tex_name,
                        xpr.to_string()
                    );

                    match xpr.format {
                        XPRFormat::ARGB => {
                            xpr.convert_argb_to_argb_lin()?;
                        }
                        XPRFormat::DXT1 => {
                            xpr.convert_dxt1_to_dxt3()?;
                        }
                        _ => {}
                    }

                    Some(xpr)
                }
            };

            if let Some(bt) = &mut base_texture {
                if let Some(mut gt) = ground_texture {
                    if bt.width == gt.width * 2 && bt.height == gt.height * 2 {
                        // Bit of a hack to deal with TEX04.BIN which has a 1024x1024 texture and a 512x512 texture
                        bt.discard_top_mipmap();
                    }
                    bt.discard_mipmaps();
                    gt.discard_mipmaps();
                    bt.extend_layers(gt)?;
                }
            } else {
                base_texture = ground_texture;
            }
        }

        let ground_texture = base_texture.expect(&format!(
            "Failed to get ground texture for mission {:02}",
            mi
        ));
        assert!(ground_texture.layers == bin_ground_texture.file_count());

        ground_textures.push(ground_texture);
    }
    game_path.pop(); // Exit BIN/

    // Setup model config database
    let mut modcfg_db = ModelConfigDB::new();

    let mut ground_lookup_pointers: [u32; 51] = [0; _];
    xbe.seek_section_offset(".data", 0x1D18)?;
    xbe.reader
        .read_u32_into::<LittleEndian>(&mut ground_lookup_pointers)?;

    build_path.push("missions");
    let _ = create_dir(&build_path);
    for mi in 0..GAME_MISSION_COUNT {
        println!("Processing mission {:02}", mi);

        /*** Processing Mission Objects ***/
        let section_name = format!("seg{:02}", mi);
        let object_buf = xbe.get_section_data(&section_name)?;

        let mut objects = OBJ::import_sb_objects(&object_buf, GAME_SCALE);
        for obj in objects.iter() {
            modcfg_db.register(&obj.modcfg);
        }

        let skybox_texture_idx = OBJ::get_sb_skybox_texture(&object_buf).expect(&format!(
            "Failed to get skybox texture for mission {:02}",
            mi
        ));

        game_path.push("BumpData");
        game_path.push(format!("MAP{:02}HIT.GAD", mi));
        let gad_path = game_path.clone();

        game_path.set_file_name(format!("MAP{:02}BMP.BMP", mi));
        let height_path = game_path.clone();

        game_path.set_file_name(format!("MAP{:02}RGB.BMP", mi));
        let texture_path = game_path.clone();
        game_path.pop();
        game_path.pop(); // Exit BumpData

        game_path.push("StgData");
        game_path.push(format!("STGDAT{:02}.STG", mi));
        let stg_path = game_path.clone();
        game_path.pop();
        game_path.pop(); // Exit StgData

        let mut mission = Mission::import_sb(
            mi,
            &gad_path,
            &height_path,
            &texture_path,
            &stg_path,
            &get_dds_object_path(skybox_texture_idx as usize),
            GAME_SCALE,
            GAME_FPS,
        )?;

        OBJ::apply_heightmap(&mut objects, &mission, 0x102); // Flags taken from function at .text+0x2e180

        let ground_tiles_texture = &mut ground_textures[mi];
        if let Some(ground) = &mut mission.ground {
            ground.apply_heightmap_holes(); // Must be done AFTER heightmap is applied to objects
            if ground_tiles_texture.layers > 1 {
                xbe.seek_pointer_offset(ground_lookup_pointers[mi])?;
                ground.create_tilemap(ground_tiles_texture.layers, &mut xbe.reader)?;
            }
        }

        build_path.push(format!("{:02}", mi));
        let _ = create_dir(&build_path);

        build_path.push("ground.dds");
        ground_tiles_texture.write_to_dds(&build_path)?;
        build_path.pop();

        let ground_texture = ground_tiles_texture.path_to_dds(godot_base_path).unwrap();
        let empty_path = PathBuf::new();
        mission.export(
            &build_path,
            &objects,
            get_gltf_path,
            [&empty_path, &ground_texture, &empty_path, &empty_path],
        )?;
        build_path.pop();
    }
    build_path.pop(); // Exit missions/ directory

    // Process additional objects
    {
        // TODO: Figure out if these are mechs
        xbe.seek_section_offset(".data", 0x128)?;

        let mut object_buf: Vec<u8> = vec![0; 92 * 48];
        xbe.reader.read_exact(&mut object_buf)?;

        let objects = OBJ::import_sb_objects(&object_buf, GAME_SCALE);
        for obj in objects.iter() {
            modcfg_db.register(&obj.modcfg);
        }
    }

    // TODO: Move weapon processing to it's own file at some point
    {
        xbe.seek_section_offset(".data", 0x82288)?;
        let mut weapon_modcfg_pointers: [u32; 86] = [0; _];
        xbe.reader
            .read_u32_into::<LittleEndian>(&mut weapon_modcfg_pointers)?;

        for &pointer in weapon_modcfg_pointers.iter() {
            xbe.seek_pointer_offset(pointer)?;

            let mut weapon_modcfg_buf: [u8; 24] = [0; _];
            xbe.reader.read_exact(&mut weapon_modcfg_buf)?;

            let weapon_modcfg = WeaponModelConfig::import_sb(&weapon_modcfg_buf);

            if let Some(mut projectile) = weapon_modcfg.projectile {
                if projectile.model == 683 && projectile.hitbox.is_none() {
                    // Fixup hitbox
                    projectile.hitbox = Some(487);
                }

                modcfg_db.register(&projectile);
            }
            if let Some(weapon) = weapon_modcfg.weapon {
                modcfg_db.register(&weapon);
            }
        }
    }

    game_path.push("BIN");

    /*** CONVERT ADVANCE TEXTURE FILES 0..30 */
    game_path.push(ADVANCE_BIN);
    let mut bin_advance = BIN::<File>::open(&game_path)?;
    game_path.pop();

    build_path.push("advance");
    let _ = create_dir(&build_path);

    println!(
        "Unpacking {} files from ADVANCE.bin",
        bin_advance.file_count()
    );

    for file_index in 0..bin_advance.file_count() {
        let file_bytes = bin_advance.file_as_bytes(file_index)?;
        if file_index < ADVANCE_TEXTURE_END {
            match XPR::import_xpr(&file_bytes) {
                Err(e) => {
                    eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                    None
                }
                Ok(mut xpr) => {
                    let tex_name = format!("{:04}", file_index);
                    println!(
                        "Converting file {:04}:{} XPR {} to DDS",
                        file_index,
                        tex_name,
                        xpr.to_string()
                    );

                    match xpr.format {
                        XPRFormat::ARGB => {
                            xpr.convert_argb_to_argb_lin()?;
                        }
                        XPRFormat::DXT1 => {
                            xpr.convert_dxt1_to_dxt3()?;
                        }
                        _ => {}
                    }

                    build_path.push(format!("{}.dds", tex_name));
                    xpr.write_to_dds(&build_path)?;
                    build_path.pop();

                    Some(xpr)
                }
            };
        } else if file_index < ADVANCE_MODEL_END {
            let model_name = format!("{:04}", file_index);
            match XBO::import(&file_bytes, GAME_SCALE) {
                Err(e) => {
                    eprintln!(
                        "Failed to convert file {:04}:{}, got error: {}",
                        file_index, model_name, e
                    );
                    None
                }
                Ok(mut xbo) => {
                    println!(
                        "Converting file {:04}:{} XBO to GLTF",
                        file_index, model_name
                    );
                    print!("{}", xbo);

                    xbo.flip_faces();

                    xbo.ignore_root_transform = true;

                    // Export GLBIN
                    build_path.push(format!("{}.glbin", model_name));
                    xbo.write_to_glbin(&build_path)?;
                    build_path.pop();

                    // Export GLTF
                    build_path.push(format!("{}.gltf", model_name));
                    xbo.write_to_gltf(&build_path)?;
                    build_path.pop();

                    Some(xbo)
                }
            };
        } else {
            let _text = import_cc_text(&file_bytes)?;
        }
    }
    build_path.pop(); // Exit advance/ directory

    /*** CONVERT OBJECT TEXTURE FILES ***/
    game_path.push(TEXOBJ_BIN);
    let mut bin_object_texture = BIN::<File>::open(&game_path)?;
    game_path.pop();

    build_path.push("textures");
    let _ = create_dir(&build_path);

    println!(
        "Unpacking {} files from TEXOBJ.bin",
        bin_object_texture.file_count()
    );

    let mut object_textures: Vec<Option<XPR>> = Vec::with_capacity(bin_object_texture.file_count());
    for file_index in 0..bin_object_texture.file_count() {
        let file_bytes = bin_object_texture.file_as_bytes(file_index)?;
        object_textures.push(match XPR::import_xpr(&file_bytes) {
            Err(e) => {
                eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                None
            }
            Ok(mut xpr) => {
                let tex_name = format!("{:04}", file_index);
                println!(
                    "Converting file {:04}:{} XPR {} to DDS",
                    file_index,
                    tex_name,
                    xpr.to_string()
                );

                match xpr.format {
                    XPRFormat::ARGB => {
                        xpr.convert_argb_to_argb_lin()?;
                    }
                    XPRFormat::DXT1 => {
                        xpr.convert_dxt1_to_dxt3()?;
                    }
                    _ => {}
                }

                build_path.push(format!("{}.dds", tex_name));
                xpr.write_to_dds(&build_path)?;
                build_path.pop();

                Some(xpr)
            }
        });
    }
    build_path.pop(); // Exit textures/ directory

    /*** CONVERT ANIMATION FILES ***/
    game_path.push(MOTION_BIN);
    let mut bin_motion = BIN::<File>::open(&game_path)?;
    game_path.pop();

    build_path.push("animations");
    let _ = create_dir(&build_path);

    println!(
        "Unpacking {} files from MOTION.bin",
        bin_motion.file_count()
    );

    // LMT and XBO files are bundled together into a GLTF
    let mut animations: Vec<Option<LMT>> = Vec::with_capacity(bin_motion.file_count());
    for file_index in 0..bin_motion.file_count() {
        let file_bytes = bin_motion.file_as_bytes(file_index)?;
        animations.push(match LMT::import(&file_bytes, GAME_SCALE, GAME_FPS) {
            Err(e) => {
                eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                None
            }
            Ok(mut lmt) => {
                let anim_name = format!("{:04}", file_index);
                println!(
                    "Converting file {:04}:{} LMT to GLTF",
                    file_index, anim_name
                );
                print!("{}", lmt);

                // Export GLBIN
                build_path.push(format!("{}.glbin", anim_name));
                lmt.write_to_glbin(&build_path, true)?;
                build_path.pop();

                // Attach a sequence to this animation if it has one
                // if let Some(seq_idx) = sequence_table[file_index] {
                //     let seq = sequences[seq_idx as usize]
                //         .as_ref()
                //         .expect("Animation is missing Sequence file");
                //     lmt.set_sequence(seq)?;
                // }

                Some(lmt)
            }
        });
    }

    build_path.pop(); // Exit animations directory

    /*** CONVERT HITBOX FILES ***/
    game_path.push(ATARI_BIN);
    let mut bin_atari = BIN::<File>::open(&game_path)?;
    game_path.pop();

    build_path.push("hitboxes");
    let _ = create_dir(&build_path);

    println!("Unpacking {} files from ATARI.bin", bin_atari.file_count());

    let mut hitboxes: Vec<Option<PPD>> = Vec::with_capacity(bin_atari.file_count());
    for file_index in 0..bin_atari.file_count() {
        let node_count_fix = match file_index {
            5 => 1,
            7 => 1,
            20 => 1,
            322 => 4,
            324 => 4,
            645 => 2,
            655 => 6,
            _ => 0,
        };

        let file_bytes = bin_atari.file_as_bytes(file_index)?;
        hitboxes.push(
            match PPD::import_scaled(&file_bytes, GAME_SCALE, node_count_fix) {
                Err(e) => {
                    eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                    None
                }
                Ok(mut ppd) => {
                    let hbx_name = format!("{:04}", file_index);
                    println!("Converting file {:04}:{} PPD to HBX", file_index, hbx_name);
                    //print!("{}", ppd);

                    build_path.push(format!("{}.glbin", hbx_name));
                    ppd.write_to_glbin(&build_path)?;
                    build_path.pop();

                    Some(ppd)
                }
            },
        );
    }

    build_path.pop(); // Exit hitboxes directory

    /*** CONVERT MODEL FILES ***/
    game_path.push(MODEL_BIN);
    let mut bin_model = BIN::<File>::open(&game_path)?;
    game_path.pop();

    build_path.push("models");
    let _ = create_dir(&build_path);

    println!("Unpacking {} files from MODEL.bin", bin_model.file_count());

    let mut models: Vec<Option<XBO>> = Vec::with_capacity(bin_model.file_count());
    for file_index in 0..bin_model.file_count() {
        let model_name = format!("{:04}", file_index);
        let file_bytes = bin_model.file_as_bytes(file_index)?;
        models.push(match XBO::import(&file_bytes, GAME_SCALE) {
            Err(e) => {
                eprintln!(
                    "Failed to convert file {:04}:{}, got error: {}",
                    file_index, model_name, e
                );
                None
            }
            Ok(mut xbo) => {
                println!(
                    "Converting file {:04}:{} XBO to GLTF",
                    file_index, model_name
                );
                print!("{}", xbo);

                xbo.flip_faces();

                xbo.ignore_root_transform = true;

                if let Some(model_config) = modcfg_db.get_model(file_index as u16) {
                    if let Some(anim_idx) = model_config.animation {
                        println!("Model {:04}, Animation {:04}", file_index, anim_idx);
                        let anim = animations[anim_idx as usize]
                            .as_ref()
                            .expect("Model is missing Animation file");
                        xbo.set_animation(anim)?;
                    }

                    if let Some(hbx_idx) = model_config.hitbox {
                        println!("Model {:04}, Hitbox {:04}", file_index, hbx_idx);
                        let hbx = hitboxes[hbx_idx as usize]
                            .as_ref()
                            .expect("Model is missing Hitbox file");
                        xbo.set_hitbox(hbx)?;
                    }

                    if let Some(tex_idx) = model_config.texture {
                        println!("Model {:04}, Texture {:04}", file_index, tex_idx);
                        let tex = object_textures[tex_idx as usize]
                            .as_ref()
                            .expect("Model is missing Texture file");
                        xbo.set_texture(tex)?;
                    }
                }

                // Export GLBIN
                build_path.push(format!("{}.glbin", model_name));
                xbo.write_to_glbin(&build_path)?;
                build_path.pop();

                // Export GLTF
                build_path.push(format!("{}.gltf", model_name));
                xbo.write_to_gltf(&build_path)?;
                build_path.pop();

                Some(xbo)
            }
        });
    }

    build_path.pop(); // Exit models/ directory

    return Ok(());
}
