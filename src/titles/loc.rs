pub mod cockpit;
pub mod mech;
pub mod wep;

use std::cmp;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::fs::create_dir;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use bmfont_rs::Chnl;
use bmfont_rs::Padding;
use bmfont_rs::Spacing;
use bmfont_rs::{ANSI, Char, Charset, Common, Font, Info};
use byteorder::ByteOrder;
use glam::Quat;
use pathdiff::diff_paths;
use serde_json::{Value, json};

use crate::bbtools::stg::Mission;
use crate::bbtools::x86::X86Context;
use crate::bbtools::*;
use crate::titles::loc::cockpit::Line;
use crate::titles::loc::cockpit::{OS, OS_SPRITE_SIZE, OSSprite};
use bin::BIN;
use eff::{EFP, LID, UV};
use lmt::{LMT, LoopMode};
use lsq::LSQ;
use obj::OBJ;
use obj::{ModelConfig, ModelConfigDB};
use ppd::PPD;
use text::{import_translations, write_translations_to_csv};
use xact::XSB;
use xbe::XBE;
use xbo::XBO;
use xpr::{XPR, XPRFormat};

use cockpit::{AMB, CBT, COC, Cockpit, CockpitLighting};
use mech::Mech;
use wep::{ProjectileCollider, Weapon, WeaponEffects, WeaponFile, WeaponType};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use glam::f32::Vec3A as Vec3;
use glam::{U8Vec4, UVec2, Vec2, Vec4Swizzles}; // Vec3A is 16-bytes so that it can function with SIMD

pub const TITLE_PREFIX: &str = "loc";
pub const TITLE_ID: u32 = 0x43430009;
const GAME_FPS: f32 = 20.0;
const GAME_SCALE: f32 = 0.01;
const GAME_TERRAIN_SCALE: f32 = 1.0 / 5000.0;
const GAME_WATER_TEXTURE_COUNT: usize = 17;
const GAME_COCKPIT_POSITION: Vec3 = Vec3::new(2000.0, 0.0, 2000.0);
const GAME_MISSION_COUNT: usize = 27; // There's partial data for 31 missions, but not enough to recover

const DISPLAY_FONT_HALF_WIDTH_CHARS: [char; 7] = [' ', '-', 'I', '_', 'i', 'j', 'l'];

const ATARI_BIN: &str = "ATARI.bin";
const LSQ_BIN: &str = "LSQ.bin";
const MODEL_BIN: &str = "MODEL.bin";
const MOTION_BIN: &str = "MOTION.bin";
const TEXTURE_BIN: &str = "TEXTURE.bin";
const VTMODEL_BIN: &str = "VTMODEL.bin";

fn get_file_name_list(bin_name: &str) -> std::str::Lines<'static> {
    match bin_name {
        ATARI_BIN => include_str!("loc/bin_file_names/ATARI.txt"),
        LSQ_BIN => include_str!("loc/bin_file_names/LSQ.txt"),
        MODEL_BIN => include_str!("loc/bin_file_names/MODEL.txt"),
        MOTION_BIN => include_str!("loc/bin_file_names/MOTION.txt"),
        TEXTURE_BIN => include_str!("loc/bin_file_names/TEXTURE.txt"),
        VTMODEL_BIN => include_str!("loc/bin_file_names/VTMODEL.txt"),
        _ => unimplemented!("Unknown bin file"),
    }
    .lines()
}

fn get_file_name(bin_name: &str, mut idx: usize) -> &'static str {
    if bin_name == VTMODEL_BIN {
        idx -= 758;
    }

    let mut fname_list = get_file_name_list(bin_name);
    return fname_list
        .nth(idx)
        .expect("Failed to find file name")
        .trim();
}

fn find_file_name(bin_name: &str, file_name: &str) -> Option<usize> {
    let mut fname_list = get_file_name_list(bin_name);
    return fname_list.position(|f| f == file_name);
}

fn get_gltf_path(idx: usize) -> PathBuf {
    let mut path = PathBuf::from("/proprietary/loc/models/");
    path.push(get_file_name(MODEL_BIN, idx));
    path.set_extension("gltf");
    return path;
}

fn get_dds_path(idx: usize) -> PathBuf {
    let mut path = PathBuf::from("/proprietary/loc/textures/");
    path.push(get_file_name(TEXTURE_BIN, idx));
    path.set_extension("dds");
    return path;
}

fn get_efg_id_path(id: usize) -> PathBuf {
    PathBuf::from(format!("/proprietary/loc/effects/effects/EFG{:04}.efg", id))
}

static EFG_ID_TABLE: OnceLock<Vec<u32>> = OnceLock::new();
fn get_efg_id(idx: u32) -> u32 {
    if idx == u32::MAX {
        return idx;
    }

    let ids = EFG_ID_TABLE.get().expect("Failed to get EFG_ID_TABLE");
    ids[idx as usize]
}

fn get_efg_idx_path(idx: usize) -> PathBuf {
    let id = get_efg_id(idx as u32);
    get_efg_id_path(id as usize)
}

fn get_efs_path(idx: usize) -> PathBuf {
    PathBuf::from(format!(
        "/proprietary/loc/effects/sequences/EFS{:02}.efs",
        idx
    ))
}

fn get_effect_spritesheet_path(idx: usize) -> PathBuf {
    PathBuf::from(format!(
        "/proprietary/loc/spritesheets/effects/EFF{:02}.spritesheet",
        idx
    ))
}

fn get_ui_spritesheet_path(idx: usize) -> PathBuf {
    PathBuf::from(format!(
        "/proprietary/loc/spritesheets/ui/UI{:02}.spritesheet",
        idx
    ))
}

fn get_cockpit_lighting_path(idx: usize) -> PathBuf {
    PathBuf::from(format!(
        "/proprietary/loc/lighting/cockpit/cl{}.cockpit_lighting",
        idx
    ))
}

fn get_effect_lighting_path(idx: usize) -> PathBuf {
    PathBuf::from(format!(
        "/proprietary/loc/lighting/effects/sp{}.point_light",
        idx
    ))
}

fn get_tracer_trail_path(idx: usize) -> PathBuf {
    PathBuf::from(format!(
        "/proprietary/loc/effects/trails/tracer/{:02}.trail",
        idx
    ))
}

fn get_smoke_trail_path(idx: usize) -> PathBuf {
    PathBuf::from(format!(
        "/proprietary/loc/effects/trails/smoke/{:02}.trail",
        idx
    ))
}

fn get_mwep_path(idx: usize) -> PathBuf {
    PathBuf::from(format!("/proprietary/loc/weapons/main/{:02}.weapon", idx))
}

fn get_swep_path(idx: usize) -> PathBuf {
    PathBuf::from(format!("/proprietary/loc/weapons/sub/{:02}.weapon", idx))
}

const EFE_MODEL_LIST: [u16; 10] = [
    1216, // EFM000
    1218, // EFM001
    1220, // EFM002
    1222, // EFM003
    1224, // EFM004
    1226, // EFM005
    1228, // EFM006
    1230, // EFM007
    1232, // EFM008
    1234, // EFM009
];

// Gathered from: BB2@0x004bdcc0
const UV_TEXTURE_LIST: [u16; 11] = [
    107, // EFFECT
    145, // CNKO0T1
    126, // HUD_00
    127, // HUD_01
    128, // HUD_02
    108, // LOOP
    318, // IDFONT00
    0,   // FACE000
    132, // IDTEX_00
    113, // RC00
    114, // RC11
];

pub fn unpack(
    xbe: &mut XBE,
    game_path: &mut PathBuf,
    build_path: &mut PathBuf,
    godot_base_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Converting LOC");

    build_path.push("loc");
    let _ = create_dir(&build_path);

    let build_title_path = build_path.clone();

    /*** CONVERT ICONS ***/
    {
        game_path.push("saveimage.xpr");
        let mut save_icon = XPR::open(&game_path)?;
        game_path.pop();

        build_path.push("saveimage.dds");
        save_icon.write_to_dds(&build_path)?;
        build_path.pop();
    }

    {
        game_path.push("titleimage.xpr");
        let mut title_icon = XPR::open(&game_path)?;
        game_path.pop();

        build_path.push("titleimage.dds");
        title_icon.write_to_dds(&build_path)?;
        build_path.pop();
    }

    /*** EXTRACT TEXT ***/
    {
        build_path.push("strings.csv");
        let translations = import_translations(xbe, ".data", 0x59D50, 1637)?;
        write_translations_to_csv(&build_path, "loc:", &translations)?;
        build_path.pop();
    }

    /*** EXTRACT SHADERS ***/
    build_path.push("shaders");
    let _ = create_dir(&build_path);
    {
        xbe.seek_section_offset(".data", 0xB840)?;
        let mut pixel_shader_pointers: [u32; 89] = [0; _];
        xbe.reader
            .read_u32_into::<LittleEndian>(&mut pixel_shader_pointers)?;

        const D3DPIXELSHADERDEF_SIZE: usize = 240;
        for (i, &pointer) in pixel_shader_pointers.iter().enumerate() {
            xbe.seek_pointer_offset(pointer)?;
            let mut buf: [u8; D3DPIXELSHADERDEF_SIZE] = [0; _];
            xbe.reader.read_exact(&mut buf)?;

            build_path.push(format!("pixel_{:02}", i));
            let file = File::create(&build_path)?;
            let mut writer = BufWriter::new(file);
            writer.write_all(&buf)?;
            build_path.pop();
        }
    }
    build_path.pop();

    game_path.push("media");

    /*** CONVERT AUDIO FILES ***/
    /*
    {
        game_path.push("sndeff");
        game_path.push("Bank.xsb");
        let xsb = XSB::open(&game_path)?;

        build_path.push("sounds");
        let _ = create_dir(&build_path);
        xsb.export_banks(&build_path)?;
        build_path.pop();

        game_path.pop();
        game_path.pop();
    }
    */

    // Load effect ID table
    {
        game_path.push("effdata");
        game_path.push("effect.tbl");

        let bytes = fs::read(&game_path)?;
        if bytes.len() % 4 != 0 {
            return Err("Effect ID table file is not a multiple of 4".into());
        }

        let mut ids: Vec<u32> = vec![0; bytes.len() / 4];
        LittleEndian::read_u32_into(bytes.as_slice(), &mut ids);

        if EFG_ID_TABLE.set(ids).is_err() {
            return Err("Failed to set effect ID table".into());
        }

        game_path.pop();
        game_path.pop();
    }

    // Setup model config database
    let mut modcfg_db = ModelConfigDB::new();

    /*** Process Mech Data ***/
    game_path.push("Eng_data");
    game_path.push("eng_data.eng");
    let mechs = Mech::import(xbe, &game_path, GAME_SCALE)?;
    game_path.pop();
    game_path.pop();

    build_path.push("mechs");
    let _ = create_dir(&build_path);

    for mech in mechs.iter() {
        // Register Chassis, Hatches, Manipulators, and Weapon Mounts
        modcfg_db.register_multiple(&mech.get_model_configs());

        build_path.push(format!("{:02}", mech.get_id()));
        let _ = create_dir(&build_path);

        mech.export(
            &build_path,
            get_gltf_path,
            get_efg_idx_path,
            get_mwep_path,
            get_swep_path,
        )?;
        build_path.pop();
    }

    build_path.pop(); // Exit mechs directory

    // SWep Box Config needs to be added manually
    modcfg_db.register(&ModelConfig {
        model: 634,
        animation: Some(7),
        sequence: Some(43),
        texture: None,
        hitbox: None,
    });

    /*** Process Weapon Data ***/
    game_path.push("weapon");
    game_path.push("wepdat.wcb");
    let weapon_file = WeaponFile::import(&game_path, GAME_SCALE, GAME_FPS)?;
    game_path.pop();
    game_path.pop();

    let rotate_left_90 = Quat::from_axis_angle(Vec3::NEG_X.to_vec3(), std::f32::consts::PI * 0.5);
    let projectile_colliders: [ProjectileCollider; 15] = [
        // 1186 = TAMA00
        ProjectileCollider::Capsule {
            radius: 0.3,
            height: 2.25,

            position: Vec3::new(0.0, 0.0, 1.125),
            rotation: rotate_left_90,
        },
        // 1188 = TAMA01
        ProjectileCollider::Capsule {
            radius: 0.06,
            height: 0.65,

            position: Vec3::new(0.0, 0.0, 0.075),
            rotation: rotate_left_90,
        },
        // 1190 = TAMA02 (Empty Model)
        ProjectileCollider::Sphere {
            radius: 0.1,
            position: Vec3::ZERO,
        },
        // 1192 = TAMA03
        ProjectileCollider::Capsule {
            radius: 0.55,
            height: 17.175,

            position: Vec3::new(0.0, 0.0, 8.975),
            rotation: rotate_left_90,
        },
        // 1194 = TAMA04
        ProjectileCollider::Box {
            size: Vec3::new(0.6, 0.75, 0.6),

            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        },
        // 1196 = TAMA05
        ProjectileCollider::Box {
            size: Vec3::new(1.0, 0.73, 0.86),

            position: Vec3::new(0.0, 0.115, 0.0),
            rotation: Quat::IDENTITY,
        },
        // 1198 = TAMA06
        ProjectileCollider::Capsule {
            radius: 0.15,
            height: 4.92,

            position: Vec3::new(0.0, 0.0, 2.46),
            rotation: rotate_left_90,
        },
        // 1200 = TAMA07
        ProjectileCollider::Capsule {
            radius: 0.25,
            height: 4.9,

            position: Vec3::new(0.0, 0.0, 2.45),
            rotation: rotate_left_90,
        },
        // 1202 = TAMA08
        ProjectileCollider::Capsule {
            radius: 0.07,
            height: 1.5,

            position: Vec3::new(0.0, 0.0, 0.8),
            rotation: rotate_left_90,
        },
        // 1204 = TAMA09
        ProjectileCollider::Box {
            size: Vec3::new(0.36, 0.4, 0.888),

            position: Vec3::new(0.0, 0.0, 0.745),
            rotation: Quat::IDENTITY,
        },
        // 1206 = TAMA10
        ProjectileCollider::Capsule {
            radius: 0.125,
            height: 4.75,

            position: Vec3::new(0.0, 0.0, 2.29),
            rotation: rotate_left_90,
        },
        // 1208 = TAMA11
        ProjectileCollider::Capsule {
            radius: 0.125,
            height: 1.4,

            position: Vec3::new(0.015, 0.0, 0.12),
            rotation: rotate_left_90,
        },
        // 1210 = TAMA12
        ProjectileCollider::Capsule {
            radius: 0.3,
            height: 7.45,

            position: Vec3::new(0.0, 0.0, 4.0),
            rotation: rotate_left_90,
        },
        // 1212 = TAMA13 (Sabot)
        ProjectileCollider::Capsule {
            radius: 0.2,
            height: 2.45,

            position: Vec3::new(0.0, 0.2, 1.25),
            rotation: rotate_left_90,
        },
        // 1214 = TAMA14
        ProjectileCollider::Capsule {
            radius: 0.15,
            height: 4.85,

            position: Vec3::new(0.0, 0.0, 2.5),
            rotation: rotate_left_90,
        },
    ];

    let mut mwep_effects: [WeaponEffects; 23] = core::array::from_fn(|_| WeaponEffects::default());
    {
        let eff = &mut mwep_effects[0];
        eff.lights = vec![1];
        eff.firing = vec![396];
        eff.mech = Some(397);
        eff.smoke = Some(418);
        eff.flare = Some(420);
        eff.tracer_trail = Some(6);
    }

    {
        let eff = &mut mwep_effects[1];
        eff.lights = vec![1];
        eff.firing = vec![404];
        eff.mech = Some(405);
        eff.flare = Some(420);
        eff.tracer_trail = Some(10);
    }

    {
        let eff = &mut mwep_effects[2];
        eff.lights = vec![1];
        eff.firing = vec![406];
        eff.mech = Some(407);
        eff.flare = Some(420);
        eff.tracer_trail = Some(11);
    }

    {
        let eff = &mut mwep_effects[3];
        eff.lights = vec![1];
        eff.firing = vec![398];
        eff.mech = Some(399);
        eff.smoke = Some(418);
        eff.flare = Some(419);
        eff.tracer_trail = Some(7);
    }

    {
        let eff = &mut mwep_effects[4];
        eff.lights = vec![1];
        eff.firing = vec![412];
        eff.mech = Some(413);
        eff.smoke = Some(418);
        eff.flare = Some(420);
        eff.tracer_trail = Some(14);
    }

    {
        let eff = &mut mwep_effects[5];
        eff.lights = vec![1];
        eff.firing = vec![408];
        eff.mech = Some(409);
        eff.flare = Some(420);
        eff.tracer_trail = Some(12);
    }

    {
        let eff = &mut mwep_effects[6];
        eff.lights = vec![1];
        eff.firing = vec![416];
        eff.mech = Some(417);
        eff.smoke = Some(418);
        eff.flare = Some(420);
        eff.tracer_trail = Some(15);
    }

    {
        let eff = &mut mwep_effects[7];
        eff.lights = vec![1];
        eff.firing = vec![400];
        eff.mech = Some(401);
        eff.smoke = Some(418);
        eff.flare = Some(419);
        eff.tracer_trail = Some(13);
    }

    {
        let eff = &mut mwep_effects[8];
        eff.lights = vec![1];
        eff.firing = vec![410];
        eff.mech = Some(411);
        eff.flare = Some(420);
        eff.tracer_trail = Some(14);
    }

    {
        let eff = &mut mwep_effects[9];
        eff.lights = vec![1];
        eff.firing = vec![402];
        eff.mech = Some(403);
        eff.smoke = Some(418);
        eff.flare = Some(419);
        eff.tracer_trail = Some(9);
    }

    {
        let eff = &mut mwep_effects[10];
        eff.lights = vec![1];
        eff.firing = vec![588];
        eff.tracer_trail = Some(19);
        eff.smoke_trail = Some(4);
    }

    {
        let eff = &mut mwep_effects[11];
        eff.lights = vec![1];
        eff.firing = vec![416];
        eff.mech = Some(417);
        eff.smoke = Some(418);
        eff.flare = Some(420);
        eff.tracer_trail = Some(16);
    }

    mwep_effects[12].lights = vec![1];

    {
        let eff = &mut mwep_effects[13];
        eff.lights = vec![0, 5];
        eff.firing = vec![460];
        eff.casing = Some(466);
        eff.tracer_trail = Some(3);
    }

    {
        let eff = &mut mwep_effects[14];
        eff.lights = vec![0, 5];
        eff.firing = vec![462];
        eff.casing = Some(468);
        eff.tracer_trail = Some(5);
    }

    {
        let eff = &mut mwep_effects[15];
        eff.lights = vec![1];
        eff.firing = vec![502];
        eff.smoke = Some(516);
        eff.flying = vec![518, 519, 520];
    }

    {
        let eff = &mut mwep_effects[16];
        eff.lights = vec![1];
        eff.firing = vec![503];
        eff.smoke = Some(516);
        eff.flying = vec![518, 519, 520];
    }

    {
        let eff = &mut mwep_effects[17];
        eff.lights = vec![1];
        eff.firing = vec![549];
        eff.smoke_trail = Some(3);
    }

    {
        let eff = &mut mwep_effects[18];
        eff.lights = vec![1];
        eff.firing = vec![550];
        eff.smoke_trail = Some(3);
    }

    {
        let eff = &mut mwep_effects[19];
        eff.lights = vec![1];
        eff.firing = vec![557];
        eff.smoke_trail = Some(1);
    }

    {
        let eff = &mut mwep_effects[20];
        eff.lights = vec![1];
        eff.firing = vec![556];
        eff.smoke_trail = Some(1);
    }

    mwep_effects[21].firing = vec![592];

    {
        let eff = &mut mwep_effects[22];
        eff.lights = vec![0, 5];
        eff.firing = vec![591];
        eff.tracer_trail = Some(18);
    }

    let mut swep_effects: [WeaponEffects; 33] = core::array::from_fn(|_| WeaponEffects::default());

    {
        let eff = &mut swep_effects[0];
        eff.lights = vec![0, 5];
        eff.firing = vec![551];
        eff.smoke_trail = Some(0);
    }

    {
        let eff = &mut swep_effects[1];
        eff.lights = vec![0, 5];
        eff.firing = vec![552];
        eff.smoke_trail = Some(0);
    }

    {
        let eff = &mut swep_effects[2];
        eff.lights = vec![0, 5];
        eff.firing = vec![551];
        eff.smoke_trail = Some(0);
    }

    {
        let eff = &mut swep_effects[3];
        eff.lights = vec![0, 5];
        eff.firing = vec![457];
        eff.casing = Some(463);
        eff.tracer_trail = Some(0);
    }

    {
        let eff = &mut swep_effects[4];
        eff.lights = vec![0, 5];
        eff.firing = vec![458];
        eff.casing = Some(464);
        eff.tracer_trail = Some(1);
    }

    {
        let eff = &mut swep_effects[5];
        eff.lights = vec![0, 5];
        eff.firing = vec![459];
        eff.casing = Some(465);
        eff.tracer_trail = Some(2);
    }

    {
        let eff = &mut swep_effects[6];
        eff.lights = vec![0, 5];
        eff.firing = vec![461];
        eff.casing = Some(467);
        eff.tracer_trail = Some(4);
    }

    {
        let eff = &mut swep_effects[7];
        eff.lights = vec![1];
        eff.firing = vec![506];
        eff.mech = Some(507);
        eff.smoke = Some(517);
        eff.smoke_trail = Some(5);
    }

    {
        let eff = &mut swep_effects[8];
        eff.lights = vec![1];
        eff.firing = vec![508];
        eff.mech = Some(509);
        eff.smoke = Some(517);
        eff.smoke_trail = Some(5);
    }

    {
        let eff = &mut swep_effects[9];
        eff.lights = vec![1];
        eff.firing = vec![593];
        eff.smoke_trail = Some(5);
    }

    {
        let eff = &mut swep_effects[10];
        eff.lights = vec![0, 5];
        eff.firing = vec![514];
        eff.smoke_trail = Some(5);
    }

    {
        let eff = &mut swep_effects[11];
        eff.lights = vec![0, 5];
        eff.firing = vec![515];
        eff.smoke_trail = Some(5);
    }

    {
        let eff = &mut swep_effects[12];
        eff.lights = vec![3];
        eff.lights_offset = Vec3::new(0.0, 0.0, 2.0);
        eff.firing = vec![640];
    }

    {
        let eff = &mut swep_effects[13];
        eff.lights = vec![3];
        eff.lights_offset = Vec3::new(0.0, 0.0, 5.0);
        eff.firing = vec![639];
    }

    {
        let eff = &mut swep_effects[13];
        eff.lights = vec![3];
        eff.lights_offset = Vec3::new(0.0, 0.0, 5.0);
        eff.firing = vec![639];
    }

    {
        let eff = &mut swep_effects[14];
        eff.lights = vec![3];
        eff.lights_offset = Vec3::new(0.0, 0.0, 5.0);
        eff.firing = vec![643];
    }

    {
        let eff = &mut swep_effects[17];
        eff.lights = vec![0, 5];
        eff.firing = vec![589];
        eff.flying = vec![520];
    }

    {
        let eff = &mut swep_effects[18];
        eff.lights = vec![0, 5];
        eff.firing = vec![590];
        eff.flying = vec![520];
    }

    swep_effects[19].firing = vec![587, 588];

    {
        let eff = &mut swep_effects[21];
        eff.lights = vec![1];
        eff.firing = vec![555];
        eff.smoke_trail = Some(2);
    }

    {
        let eff = &mut swep_effects[22];
        eff.lights = vec![0, 5];
        eff.firing = vec![586];
    }

    {
        let eff = &mut swep_effects[23];
        eff.lights = vec![1];
        eff.firing = vec![512, 398];
        eff.mech = Some(513);
        eff.smoke = Some(517);
        eff.smoke_trail = Some(5);
    }

    {
        let eff = &mut swep_effects[24];
        eff.firing = vec![646];
        eff.mech = Some(647);
        eff.charging = Some(645);
        eff.flying = vec![656];
    }

    {
        let eff = &mut swep_effects[25];
        eff.lights = vec![0, 5];
        eff.firing = vec![554];
        eff.smoke_trail = Some(0);
    }

    {
        let eff = &mut swep_effects[26];
        eff.firing = vec![646];
        eff.mech = Some(647);
        eff.charging = Some(645);
        eff.flying = vec![656];
    }

    {
        let eff = &mut swep_effects[27];
        eff.lights = vec![1];
        eff.firing = vec![510];
        eff.mech = Some(511);
        eff.smoke = Some(517);
        eff.smoke_trail = Some(5);
    }

    {
        let eff = &mut swep_effects[28];
        eff.lights = vec![1];
        eff.firing = vec![504];
        eff.mech = Some(505);
        eff.smoke = Some(517);
        eff.smoke_trail = Some(5);
    }

    {
        let eff = &mut swep_effects[31];
        eff.firing = vec![553];
        eff.smoke_trail = Some(0);
    }

    for (eff, wep) in mwep_effects.iter_mut().zip(&weapon_file.mweps) {
        eff.thrust = wep.get_thrust_effect();
        eff.mech_impact = wep.get_mech_impact_effect();
    }

    for (eff, wep) in swep_effects.iter_mut().zip(&weapon_file.sweps) {
        eff.thrust = wep.get_thrust_effect();
        eff.mech_impact = wep.get_mech_impact_effect();
    }

    let mweps = Weapon::import(
        xbe,
        weapon_file.mweps,
        WeaponType::MWEP,
        mwep_effects.as_slice(),
        weapon_file.tracer_trails.as_slice(),
        projectile_colliders.as_slice(),
    )?;
    let mut sweps = Weapon::import(
        xbe,
        weapon_file.sweps,
        WeaponType::SWEP,
        swep_effects.as_slice(),
        weapon_file.tracer_trails.as_slice(),
        projectile_colliders.as_slice(),
    )?;

    // Fixup SWEP flags
    sweps[29].weapon_modcfg.flags |= 0x0002_0000; // Add attack animation for Gauss
    sweps[30].weapon_modcfg.flags &= !0x0002_0000; // Remove attack animation for Zug shield

    //let cweps = Weapon::import(xbe, weapon_file.cweps, WeaponType::CWEP)?;

    // const WEP_MASK: u32 = 0x0200_0000;

    // println!("SWEP Model Flags:");
    // for mwep in mweps.iter() {
    //     if mwep.weapon_modcfg.flags == 0 {
    //         continue;
    //     }
    //     println!(
    //         "  Flags: {:08X}, Mask: {}, Name: {}",
    //         mwep.weapon_modcfg.flags,
    //         mwep.weapon_modcfg.flags & WEP_MASK == WEP_MASK,
    //         mwep.name
    //     )
    // }

    // println!("MWEP Model Flags:");
    // for swep in sweps.iter() {
    //     if swep.weapon_modcfg.flags == 0 {
    //         continue;
    //     }
    //     println!(
    //         "  Flags: {:08X}, Mask: {}, Name: {}",
    //         swep.weapon_modcfg.flags,
    //         swep.weapon_modcfg.flags & WEP_MASK == WEP_MASK,
    //         swep.name
    //     )
    // }

    build_path.push("weapons");
    let _ = create_dir(&build_path);
    {
        build_path.push("main");
        let _ = create_dir(&build_path);

        // Register Main Weapon Models
        for wep in mweps.iter() {
            modcfg_db.register_multiple(&wep.get_model_configs());

            wep.export(
                &build_path,
                get_gltf_path,
                get_efg_idx_path,
                get_effect_lighting_path,
                get_tracer_trail_path,
                get_smoke_trail_path,
            )?;
        }

        build_path.pop(); // Exit main directory

        build_path.push("sub");
        let _ = create_dir(&build_path);

        // Register Sub Weapon Models
        for wep in sweps.iter() {
            modcfg_db.register_multiple(&wep.get_model_configs());

            wep.export(
                &build_path,
                get_gltf_path,
                get_efg_idx_path,
                get_effect_lighting_path,
                get_tracer_trail_path,
                get_smoke_trail_path,
            )?;
        }

        build_path.pop(); // Exit sub directory
    }
    build_path.pop(); // Exit weapons directory

    build_path.push("effects");
    let _ = create_dir(&build_path);

    build_path.push("trails");
    let _ = create_dir(&build_path);
    {
        build_path.push("smoke");
        let _ = create_dir(&build_path);

        for (i, trail) in weapon_file.smoke_trails.iter().enumerate() {
            build_path.push(format!("{:02}.trail", i));
            trail.export(&build_path, get_effect_spritesheet_path)?;
            build_path.pop();

            println!("Smoke Trail {} - {}", i, trail);
        }

        build_path.pop(); // Exit smoke directory
        build_path.push("tracer");
        let _ = create_dir(&build_path);

        for (i, trail) in weapon_file.tracer_trails.iter().enumerate() {
            build_path.push(format!("{:02}.trail", i));
            trail.export(&build_path, get_effect_spritesheet_path)?;
            build_path.pop();

            println!("Tracer Trail {} - {}", i, trail);
        }

        build_path.pop(); // Exit tracer directory
    }
    build_path.pop(); // Exit trails directory

    /*** Process Effects ***/
    game_path.push("effdata");
    game_path.push("effect.efp");
    let efp = EFP::import(
        &game_path,
        EFE_MODEL_LIST.as_slice(),
        UV_TEXTURE_LIST.as_slice(),
        EFG_ID_TABLE
            .get()
            .expect("Failed to get EFG_ID_TABLE")
            .as_slice(),
        GAME_SCALE,
        GAME_FPS,
    )?;

    /*
    game_path.set_extension("tbl");

    build_path.push("ids.tbl");
    fs::copy(&game_path, &build_path)?;
    build_path.pop();
    */

    game_path.pop(); // Remove effect.tbl
    game_path.pop(); // Exit effdata directory

    build_path.push("effects");
    let _ = create_dir(&build_path);

    for efe in efp.efe_list.iter() {
        build_path.push(format!("EFG{:04}.efg", efe.id));
        efe.export(
            &build_path,
            get_efg_id_path,
            get_effect_spritesheet_path,
            get_efs_path,
            get_gltf_path,
        )?;
        build_path.pop();
    }

    build_path.pop(); // Exit efx directory

    build_path.push("sequences");
    let _ = create_dir(&build_path);

    for seq in efp.seq_list.iter() {
        build_path.push(format!("EFS{:02}.efs", seq.idx));
        seq.export(&build_path)?;
        build_path.pop();
    }

    build_path.pop(); // Exit effects/sequence directory

    build_path.push("surfaces");
    let _ = create_dir(&build_path);

    build_path.push("common.tbl");
    {
        xbe.seek_section_offset(".data", 0x37F5C)?;

        let mut surface_effect_table: [u32; 28 * 32] = [0; _];
        xbe.reader
            .read_u32_into::<LittleEndian>(&mut surface_effect_table)?;

        // Fixup the overlapping data
        surface_effect_table[..9].fill(0);

        let file = File::create(&build_path)?;
        let mut writer = BufWriter::new(file);

        for surface_effect_idx in surface_effect_table {
            writer.write_u32::<LittleEndian>(get_efg_id(surface_effect_idx))?;
        }
    }
    build_path.pop();

    let mut mech_surface_effect_paths: Vec<PathBuf> = Vec::with_capacity(34);
    {
        xbe.seek_section_offset(".data", 0x38D80)?;

        const MECH_SURFACE_EFFECT_COUNT: usize = 28 * 9;
        let mut mech_surface_effect_table: [u32; 34 * MECH_SURFACE_EFFECT_COUNT] = [0; _];
        xbe.reader
            .read_u32_into::<LittleEndian>(&mut mech_surface_effect_table)?;

        let (mech_surface_effect_chunks, []) =
            mech_surface_effect_table.as_chunks::<MECH_SURFACE_EFFECT_COUNT>()
        else {
            panic!("mech_surface_effect_table length not a multiple of MECH_SURFACE_EFFECT_COUNT");
        };

        for (i, mech_surface_effects) in mech_surface_effect_chunks.iter().enumerate() {
            build_path.push(format!("mech{:02}.tbl", i));
            mech_surface_effect_paths.push(build_path.clone());

            let file = File::create(&build_path)?;
            let mut writer = BufWriter::new(file);

            for &surface_effect_idx in mech_surface_effects {
                writer.write_u32::<LittleEndian>(get_efg_id(surface_effect_idx))?;
            }

            build_path.pop();
        }
    };

    build_path.pop(); // Exit effects/surfaces directory

    {
        let mut water_bump_texture: Option<XPR> = None;
        game_path.push("BumpData");
        for i in 0..GAME_WATER_TEXTURE_COUNT {
            game_path.push(format!("b{:02}.raw", i));
            let mut xpr = XPR::open(&game_path)?;
            game_path.pop();

            xpr.convert_gb_to_gb_lin()?;

            println!("Importing Water Bump XRAW {:02}: {}", i, xpr);

            if let Some(wbt) = &mut water_bump_texture {
                wbt.extend_layers(xpr)?;
            } else {
                water_bump_texture = Some(xpr);
            }
        }
        game_path.pop();

        build_path.push("water_bump.dds");
        water_bump_texture.unwrap().write_to_dds(&build_path)?;
        build_path.pop();
    }

    build_path.pop(); // Exit effects directory

    build_path.push("lighting");
    let _ = create_dir(&build_path);

    game_path.push("lid");
    game_path.push("eff.lid");
    // twep.lid is not used
    let lid_list = LID::import(&game_path, GAME_SCALE, GAME_FPS)?;
    game_path.pop();
    game_path.pop();

    build_path.push("effects");
    let _ = create_dir(&build_path);

    for lid in lid_list.iter() {
        build_path.push(format!("sp{}.point_light", lid.idx));
        lid.export(&build_path)?;
        build_path.pop();

        println!("Point Light - {}", lid);
    }
    build_path.pop(); // Exit lighting/effects directory

    game_path.push("cockpit");

    game_path.push("PLPACK.coc");
    let coc_list = COC::import_bin(&game_path, GAME_SCALE, GAME_COCKPIT_POSITION)?;
    game_path.pop();

    for coc in coc_list.iter() {
        println!("{}", coc);
    }

    game_path.push("AMBPACK.amb");
    let amb_list = AMB::import_bin(&game_path)?;
    game_path.pop();

    for amb in amb_list.iter() {
        println!("{}", amb);
    }

    game_path.push("BTLPACK.cbt");
    let cbt_list = CBT::import_bin(&game_path, GAME_SCALE, GAME_COCKPIT_POSITION, GAME_FPS)?;
    game_path.pop();

    for cbt in cbt_list.iter() {
        println!("{}", cbt);
    }

    build_path.push("cockpit");
    {
        let _ = create_dir(&build_path);

        let cockpit_closed_ticks = [114, 87, 110, 88, 113, 90];

        println!("Build Cockpit Lighting");
        assert!(coc_list.len() == amb_list.len());
        for c in 0..6 {
            let mut cl = CockpitLighting::default();
            cl.cockpit_closed_time = cockpit_closed_ticks[c] as f32 / GAME_FPS;

            let c3 = c * 3;
            // Not all CBTs are used
            let cbt0 =
                CBT::find_by_idx(&cbt_list, c3).expect("Failed to find cockpit lighting Anim_0");
            let cbt1 = CBT::find_by_idx(&cbt_list, c3 + 1)
                .expect("Failed to find cockpit lighting Anim_1");
            let cbt2 = CBT::find_by_idx(&cbt_list, c3 + 2)
                .expect("Failed to find cockpit lighting Anim_2");

            cl.add_cbt(cbt0);
            cl.add_cbt(cbt1);
            cl.add_cbt(cbt2);

            cl.add_amb_coc(&amb_list[c + 6], &coc_list[c + 6]);
            cl.add_amb_coc(&amb_list[c], &coc_list[c]);

            let cbt5 = CBT::find_by_idx(&cbt_list, c3 + 21)
                .expect("Failed to find cockpit lighting Anim_5");
            cl.add_cbt(cbt5);

            println!(
                "CL{} | Anim_0: CBT{:02}, Anim_1: CBT{:02}, Anim_2: CBT{:02}, Anim_3: AMB/COC{:02}, Anim_4: AMB/COC{:02}, Anim_5: CBT{:02}",
                c,
                cbt0.idx,
                cbt1.idx,
                cbt2.idx,
                c,
                c + 6,
                cbt5.idx,
            );

            build_path.push(format!("cl{}.cockpit_lighting", c));
            cl.export(&build_path)?;
            build_path.pop();
        }
    }
    build_path.pop(); // Exit lighting/cockpit directory
    build_path.pop(); // Exit lighting directory

    build_path.push("spritesheets");
    let _ = create_dir(&build_path);

    build_path.push("effects");
    let _ = create_dir(&build_path);

    for uv in efp.uv1_list.iter() {
        build_path.push(format!("EFF{:02}.spritesheet", uv.idx));
        uv.export(&build_path, get_dds_path)?;
        build_path.pop();
    }

    build_path.pop(); // Exit spritesheets/effects directory

    build_path.push("ui");
    let _ = create_dir(&build_path);

    for uv in efp.uv2_list.iter() {
        build_path.push(format!("UI{:02}.spritesheet", uv.idx));
        uv.export(&build_path, get_dds_path)?;
        build_path.pop();
    }

    build_path.pop(); // Exit spritesheets/ui directory

    build_path.push("cockpit");
    let _ = create_dir(&build_path);
    {
        let display_uv_data = [
            ("coko0uv.uv", 145), // CNKO0T1
            ("coko1uv.uv", 147), // CNKO1T1
            ("coko2uv.uv", 149), // CNKO2T1
            ("cowm1uv.uv", 151), // CNWM1T1
            ("coly0uv.uv", 153), // CNLY0T1
            ("coak1uv.uv", 155), // CNAK1T1
        ];

        for (cup, cut) in display_uv_data {
            game_path.push(cup);
            let display_uv = UV::import_display(&game_path, cut, Vec2::new(512.0, 256.0))?;
            game_path.pop();

            build_path.push(cup);
            build_path.set_extension("spritesheet");
            display_uv.export(&build_path, get_dds_path)?;
            build_path.pop();
        }
    }
    build_path.pop(); // Exit sprites/cockpit directory

    build_path.pop(); // Exit sprites directory

    build_path.push("cockpits");
    let _ = create_dir(&build_path);

    let cockpits = Cockpit::import(xbe, GAME_SCALE, GAME_COCKPIT_POSITION)?;
    for cockpit in cockpits.iter() {
        modcfg_db.register_multiple(&cockpit.get_model_configs());

        cockpit.export(
            &build_path,
            get_dds_path,
            get_gltf_path,
            get_cockpit_lighting_path,
            get_efg_idx_path,
        )?;
    }

    build_path.pop(); // Exit cockpits directory

    build_path.push("os");
    let _ = create_dir(&build_path);
    {
        build_path.push("boot");
        let _ = create_dir(&build_path);
        {
            let boot_common = Line::import_linesdefs(xbe, ".data", 0x451B8, 38)?;

            build_path.push("common.lines");
            let lines_path = diff_paths(&build_path, godot_base_path).unwrap();
            Line::export_linesdefs(&boot_common, &build_path)?;
            build_path.pop();

            let strings_path;
            // Copy boot strings
            {
                game_path.push("os.str");
                let in_file = File::open(&game_path)?;
                let mut reader = BufReader::new(in_file);
                game_path.pop();

                build_path.push("strings.txt");
                strings_path = diff_paths(&build_path, godot_base_path).unwrap();
                let out_file = File::create(&build_path)?;
                let mut writer = BufWriter::new(out_file);
                build_path.pop();

                // Drop the first line because it just contains the line count
                let mut line_count_str = String::new();
                reader.read_line(&mut line_count_str)?;

                // Copy the rest of the file
                io::copy(&mut reader, &mut writer)?;
            }

            // Switches
            let switch_error_color = U8Vec4::new(0xFF, 0x50, 0x00, 0xFF);
            let switch_primary_color = U8Vec4::new(0x00, 0xFF, 0xAA, 0xFF);

            let switch_progress_vertices: [Vec2; 80];
            let switch_success_vertices: [UVec2; 15];
            let switch_error_vertices: [Vec2; 30];
            {
                let mut x86_ctx = X86Context::new(4);

                xbe.seek_section_offset(".text", 0x61420)?;
                let mut bytes_read = 0;
                while bytes_read < 2674 {
                    bytes_read += x86_ctx.execute_instruction(xbe)?;
                }

                let stack = x86_ctx.get_stack();

                switch_progress_vertices = {
                    let mut components: [f32; 160] = [0.0; _];
                    LittleEndian::read_f32_into(&stack[36..676], &mut components);
                    core::array::from_fn(|i| Vec2::from_slice(&components[i * 2..i * 2 + 2]))
                };
                //println!("{:?}", switch_progress_vertices);

                switch_success_vertices = {
                    let mut components: [u32; 30] = [0; _];
                    LittleEndian::read_u32_into(&stack[676..796], &mut components);
                    core::array::from_fn(|i| UVec2::from_slice(&components[i * 2..i * 2 + 2]))
                };
                //println!("{:?}", switch_success_vertices);

                switch_error_vertices = {
                    let mut components: [f32; 60] = [0.0; _];
                    LittleEndian::read_f32_into(&stack[796..1036], &mut components);
                    core::array::from_fn(|i| Vec2::from_slice(&components[i * 2..i * 2 + 2]))
                };
                //println!("{:?}", switch_error_vertices);
            }

            let startup_progress_vertices: [Vec2; 15];
            {
                let mut x86_ctx = X86Context::new(176);

                xbe.seek_section_offset(".text", 0x60784)?;
                let mut bytes_read = 0;
                while bytes_read < 267 {
                    bytes_read += x86_ctx.execute_instruction(xbe)?;
                }

                let stack = x86_ctx.get_stack();

                startup_progress_vertices = {
                    let mut components: [f32; 30] = [0.0; _];
                    LittleEndian::read_f32_into(&stack[44..164], &mut components);
                    core::array::from_fn(|i| Vec2::from_slice(&components[i * 2..i * 2 + 2]))
                };
            }

            game_path.push("OS.os");
            let os_list = OS::import_bin(&game_path, GAME_FPS)?;
            game_path.pop();

            for (i, os) in os_list.iter().enumerate() {
                println!("OS File: {:02} | {}", i, os);
            }

            let startup_lines_offsets: [(u32, usize); 3] =
                [(0x33600, 6), (0x33CB0, 7), (0x34588, 7)];

            let startup_sprites_offsets: [(u32, usize); 3] =
                [(0x37310, 15), (0x376D0, 16), (0x37AD0, 15)];

            let mut boot_color_pointers: [u32; 8] = [0; _];
            boot_color_pointers[0] = 0x256654;

            xbe.seek_section_offset(".text", 0x5FAF8)?;
            for ptr in boot_color_pointers.iter_mut().skip(1) {
                *ptr = xbe.reader.read_u32::<LittleEndian>()?;
                xbe.reader.seek_relative(1)?;
            }

            let spritesheet_path = get_ui_spritesheet_path(4);

            for g in 0..3 {
                build_path.push(format!("gen{}", g + 1));
                let _ = create_dir(&build_path);

                let texture_path = get_dds_path(126 + g);

                let mut font_path = build_title_path.clone();
                font_path.push("fonts");
                font_path.push(format!("gen{}.fnt", g + 1));

                font_path = diff_paths(&font_path, godot_base_path).unwrap();

                // Boot animtions
                for m in 0..6 {
                    let os_idx = m * 6 + g * 2;

                    build_path.push(format! {"Ignition_{}.boot_anim", m});
                    os_list[os_idx].export(
                        &build_path,
                        &font_path,
                        &strings_path,
                        &texture_path,
                        &spritesheet_path,
                        &lines_path,
                    )?;
                    build_path.pop();

                    build_path.push(format! {"Activation_{}.boot_anim", m});
                    os_list[os_idx + 1].export(
                        &build_path,
                        &font_path,
                        &strings_path,
                        &texture_path,
                        &spritesheet_path,
                        &lines_path,
                    )?;
                    build_path.pop();
                }

                {
                    // Switches
                    let error_vertices = &switch_error_vertices[g * 10..g * 10 + 10]; // Start & End vertices
                    let progress_vertices = &switch_progress_vertices[g * 20..]; // 4 vertices for the quad
                    let success_vertices = &switch_success_vertices[g * 5..g * 5 + 5]; // 1 vertex for the position

                    build_path.push("Systems.boot_systems");
                    let file = File::create(&build_path)?;
                    build_path.pop();

                    let mut writer = BufWriter::new(file);
                    writer.write_u32::<LittleEndian>(5)?; // System count

                    writer.write_all(switch_error_color.to_array().as_slice())?;
                    writer.write_all(switch_primary_color.to_array().as_slice())?;

                    for v in error_vertices.iter() {
                        writer.write_f32::<LittleEndian>(v.x)?;
                        writer.write_f32::<LittleEndian>(v.y)?;
                    }

                    let switch_quad_count: usize = if g == 2 { 2 } else { 1 };
                    writer.write_u32::<LittleEndian>(switch_quad_count as u32)?; // Quads per switch

                    for v in progress_vertices[..switch_quad_count * 20].iter() {
                        writer.write_f32::<LittleEndian>(v.x)?;
                        writer.write_f32::<LittleEndian>(v.y)?;
                    }

                    let success_text = if g == 0 { "*" } else { "OK" };
                    write_pascal_string(success_text, &mut writer)?;
                    for v in success_vertices.iter() {
                        writer.write_f32::<LittleEndian>(v.x as f32)?;
                        writer.write_f32::<LittleEndian>(v.y as f32)?;
                    }

                    // Startup
                    let startup_vertices = &startup_progress_vertices[g * 5..g * 5 + 5]; // 1 vertex for the position

                    writer.write_u32::<LittleEndian>(switch_quad_count as u32)?; // Quads per switch
                    for v in startup_vertices.iter() {
                        writer.write_f32::<LittleEndian>(v.x as f32)?;
                        writer.write_f32::<LittleEndian>(v.y as f32)?;
                    }
                    if switch_quad_count == 2 {
                        for v in startup_vertices.iter() {
                            writer.write_f32::<LittleEndian>(v.x as f32 + 403.0)?;
                            writer.write_f32::<LittleEndian>(v.y as f32)?;
                        }
                    }
                }

                // Startup screen
                {
                    let (sl_offset, sl_count) = startup_lines_offsets[g];
                    let startup_lines = Line::import_linesdefs(xbe, ".data", sl_offset, sl_count)?;

                    build_path.push("start.lines");
                    Line::export_linesdefs(&startup_lines, &build_path)?;
                    build_path.pop();
                }

                {
                    let (spr_offset, spr_count) = startup_sprites_offsets[g];
                    let sprites = OSSprite::import_multiple(
                        xbe,
                        ".data",
                        spr_offset,
                        spr_count,
                        boot_color_pointers.as_slice(),
                    )?;

                    build_path.push("start.sprites");
                    OSSprite::export_multiple(
                        &build_path,
                        sprites.as_slice(),
                        get_ui_spritesheet_path,
                    )?;
                    build_path.pop();
                }

                build_path.pop(); // Exit os/gen# directory
            }
        }
        build_path.pop();

        build_path.push("hud");
        let _ = create_dir(&build_path);
        {
            let mut hud_color_pointers: [u32; 6] = [0; _];

            xbe.seek_section_offset(".text", 0x27B56)?;
            for i in 0..5 {
                hud_color_pointers[i] = xbe.reader.read_u32::<LittleEndian>()?;
                xbe.reader.seek_relative(6)?;
            }

            xbe.seek_section_offset(".text", 0x27C8E)?;
            hud_color_pointers[5] = xbe.reader.read_u32::<LittleEndian>()?;

            for c in 0..6 {
                build_path.push(format!("cockpit{}", c));
                let _ = create_dir(&build_path);

                {
                    const HUD_LINESDEFS_COUNT: usize = 74;
                    let hud_lines = Line::import_linesdefs(
                        xbe,
                        ".data",
                        0x2D5F0 + 8 * (c * HUD_LINESDEFS_COUNT) as u32,
                        HUD_LINESDEFS_COUNT,
                    )?;

                    build_path.push("hud.lines");
                    Line::export_linesdefs(&hud_lines, &build_path)?;
                    build_path.pop();
                }

                {
                    const HUD_SPRITE_COUNT: usize = 30;
                    let hud_sprites = OSSprite::import_multiple(
                        xbe,
                        ".data",
                        0x2E980 + (OS_SPRITE_SIZE * HUD_SPRITE_COUNT * c) as u32,
                        HUD_SPRITE_COUNT,
                        &hud_color_pointers,
                    )?;

                    build_path.push("hud.sprites");
                    OSSprite::export_multiple(&build_path, &hud_sprites, get_ui_spritesheet_path)?;
                    build_path.pop();
                }

                build_path.pop();
            }

            build_path.push("palettes");
            let _ = create_dir(&build_path);
            {
                const HUD_PALETTE_SIZE: usize = 5;
                const HUD_PALETTE_COUNT: usize = 5;
                let hud_colors_offsets: [u32; HUD_PALETTE_COUNT] =
                    [0x27B5A, 0x27B91, 0x27BC8, 0x27BFC, 0x27C30];
                let mut hud_palettes: Vec<[U8Vec4; HUD_PALETTE_SIZE]> =
                    Vec::with_capacity(HUD_PALETTE_COUNT);
                for offset in hud_colors_offsets {
                    xbe.seek_section_offset(".text", offset)?;
                    let mut palette: [U8Vec4; HUD_PALETTE_SIZE] = [U8Vec4::ZERO; _];
                    for i in 0..HUD_PALETTE_SIZE {
                        let mut palette_buf: [u8; 4] = [0; _];
                        xbe.reader.read_exact(&mut palette_buf)?;
                        xbe.reader.seek_relative(6)?;

                        palette[i] = U8Vec4::from_array(palette_buf).zyxw();
                    }
                    hud_palettes.push(palette);
                }

                for (i, palette) in hud_palettes.iter().enumerate() {
                    build_path.push(format!("{}.palette", i));
                    let file = File::create(&build_path)?;
                    let mut writer = BufWriter::new(file);
                    build_path.pop();

                    writer.write_u32::<LittleEndian>(palette.len() as u32)?;
                    for color in palette {
                        let color_norm = color.as_vec4() / 255.0;
                        for v in color_norm.to_array() {
                            writer.write_f32::<LittleEndian>(v)?;
                        }
                    }
                }
            }
            build_path.pop();
        }
        build_path.pop();
    }
    build_path.pop(); // Exit os directory

    game_path.pop(); // Exit cockpit directory

    {
        let mut terrain_resistances: [f32; 64] = [0.0; _];
        xbe.seek_section_offset(".data", 0x55728)?;
        xbe.reader
            .read_f32_into::<LittleEndian>(&mut terrain_resistances)?;

        build_path.push("terrain_resistances.tbl");
        let tr_file = File::create(&build_path)?;
        let mut tr_writer = BufWriter::new(tr_file);
        for &tr in terrain_resistances.iter().step_by(2) {
            tr_writer.write_f32::<LittleEndian>(tr * 900.0)?;
        }
        build_path.pop();
    }

    build_path.push("missions");
    let _ = create_dir(&build_path);

    {
        let mut mission_ids: [u32; GAME_MISSION_COUNT] = [u32::MAX; _];
        xbe.seek_section_offset(".data", 0x59518)?;
        xbe.reader.read_u32_into::<LittleEndian>(&mut mission_ids)?;

        {
            build_path.push("mission_ids.tbl");
            let file = File::create(&build_path)?;
            let mut writer = BufWriter::new(file);

            for mid in mission_ids {
                writer.write_u8(mid as u8)?;
            }
            build_path.pop();
        }

        let mut gad_path = game_path.clone();
        gad_path.push("BumpData");

        game_path.push("StgData");
        for mi in 0..GAME_MISSION_COUNT {
            println!(
                "Processing mission {:02}, ID: {:02}",
                mi, mission_ids[mi] as u8
            );

            /*** Processing Mission Objects ***/
            let section_name = format!("seg{:02}", mi);
            let object_buf = xbe.get_section_data(&section_name)?;

            let mut objects = OBJ::import_loc_objects(&object_buf, GAME_SCALE);

            game_path.push(format!("rstart{:02}.rst", mi));
            if game_path.is_file() {
                OBJ::apply_rstart(&mut objects, &game_path, GAME_SCALE)?;
            }
            game_path.pop();

            for obj in objects.iter() {
                modcfg_db.register(&obj.modcfg);
            }

            /*** Processing stage datat ***/
            let stg_paths: [PathBuf; 4] = core::array::from_fn(|i| {
                let mut p = game_path.clone();
                p.push(format!("std{}_{:02}.stg", i, mi));
                p
            });

            let mut gnd_path = gad_path.clone();
            gnd_path.push(format!("map{:02}.gnd", mi));
            gad_path.push(format!("map{:02}hit.gad", cmp::min(mi, 24))); // Fix an issue where maps 25/26 need to use map 24's .gad
            let mut mission = Mission::import_loc(
                mi,
                &gad_path,
                &gnd_path,
                stg_paths.each_ref().map(|p| p.as_path()),
                [
                    &get_dds_path(265 + mi * 2), // Sky 0
                    &get_dds_path(266 + mi * 2), // Sky 1
                ],
                GAME_TERRAIN_SCALE,
                GAME_SCALE,
                GAME_FPS,
            )?;
            gad_path.pop();

            OBJ::apply_heightmap(&mut objects, &mission, 0x1);

            build_path.push(format!("{:02}", mi));
            let _ = create_dir(&build_path);
            {
                let dds_paths = [
                    get_dds_path(157 + mi), // Object
                    get_dds_path(184 + mi), // Terrain
                    get_dds_path(211 + mi), // Map Big
                    get_dds_path(238 + mi), // Map Small
                ];

                mission.export(
                    &build_path,
                    &objects,
                    get_gltf_path,
                    dds_paths.each_ref().map(|p| p.as_path()),
                )?;
            }
            build_path.pop();
        }
        game_path.pop(); // Exit StgData directory
    }

    build_path.pop(); // Exit missions directory

    game_path.push("bin");

    /*** CONVERT TEXTURE FILES ***/
    game_path.push(TEXTURE_BIN);
    let mut bin_texture = BIN::<File>::open(&game_path)?;
    game_path.pop();

    build_path.push("textures");
    let _ = create_dir(&build_path);

    println!(
        "Unpacking {} files from TEXTURE.bin",
        bin_texture.file_count()
    );

    let mut textures: Vec<Option<XPR>> = Vec::with_capacity(bin_texture.file_count());
    for file_index in 0..bin_texture.file_count() {
        let file_bytes = bin_texture.file_as_bytes(file_index)?;
        textures.push(match XPR::import_xpr(&file_bytes[..]) {
            Err(e) => {
                eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                None
            }
            Ok(mut xpr) => {
                let tex_name = get_file_name(TEXTURE_BIN, file_index);
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

    build_path.pop(); // Exit textures directory

    /*** CONVERT SEQUENCE FILES ***/
    game_path.push(LSQ_BIN);
    let mut bin_lsq = BIN::<File>::open(&game_path)?;
    game_path.pop();

    build_path.push("sequences");
    let _ = create_dir(&build_path);

    println!("Unpacking {} files from ATARI.bin", bin_lsq.file_count());

    let mut sequences: Vec<Option<LSQ>> = Vec::with_capacity(bin_lsq.file_count());
    for file_index in 0..bin_lsq.file_count() {
        let file_bytes = bin_lsq.file_as_bytes(file_index)?;
        sequences.push(match LSQ::import(&file_bytes, GAME_SCALE, GAME_FPS) {
            Err(e) => {
                eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                None
            }
            Ok(mut lsq) => {
                let seq_name = get_file_name(LSQ_BIN, file_index);
                println!("Converting file {:04}:{} LSQ to SEQ", file_index, seq_name);
                print!("{}", lsq);

                if file_index == 26 || file_index == 31 || file_index == 40 {
                    // These sequence files have an animation count that's off by one, fix it
                    lsq.animation_count += 1;
                }

                build_path.push(format!("{}.glbin", seq_name));
                lsq.write_to_glbin(&build_path)?;
                build_path.pop();

                Some(lsq)
            }
        });
    }

    build_path.pop(); // Exit sequences directory

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

    let sequence_table = modcfg_db.get_sequence_table(bin_motion.file_count());
    for (i, sequence) in sequence_table.iter().enumerate() {
        println!(
            "Animation {} - Sequence {:?}",
            get_file_name(MOTION_BIN, i),
            sequence
        );
    }

    // LMT and XBO files are bundled together into a GLTF
    let mut animations: Vec<Option<LMT>> = Vec::with_capacity(bin_motion.file_count());
    for file_index in 0..bin_motion.file_count() {
        let file_bytes = bin_motion.file_as_bytes(file_index)?;
        animations.push(match LMT::import(&file_bytes[..], GAME_SCALE, GAME_FPS) {
            Err(e) => {
                eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                None
            }
            Ok(mut lmt) => {
                let anim_name = get_file_name(MOTION_BIN, file_index);
                println!(
                    "Converting file {:04}:{} LMT to GLTF",
                    file_index, anim_name
                );
                print!("{}", lmt);

                if file_index >= 11 && file_index <= 16 {
                    // Set loop for melee attacks
                    lmt.set_loop_mode(1, LoopMode::Linear)?; // Attack Loop
                }

                if file_index == 18 {
                    // Set loop for jar shield melee attack
                    lmt.set_loop_mode(3, LoopMode::Linear)?; // Attack Loop
                }

                if file_index >= 20 && file_index <= 54 {
                    // Set loop modes for mech chassis animations
                    lmt.set_loop_mode(1, LoopMode::Linear)?; // Walk
                    lmt.set_loop_mode(2, LoopMode::Linear)?; // Run
                    lmt.set_loop_mode(3, LoopMode::Linear)?; // Wheel
                    lmt.set_loop_mode(4, LoopMode::Linear)?; // Reverse
                }

                // Export GLBIN
                build_path.push(format!("{}.glbin", anim_name));
                lmt.write_to_glbin(&build_path, true)?;
                build_path.pop();

                // Attach a sequence to this animation if it has one
                if let Some(seq_idx) = sequence_table[file_index] {
                    let seq = sequences[seq_idx as usize]
                        .as_ref()
                        .expect("Animation is missing Sequence file");
                    lmt.set_sequence(seq)?;
                }

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
            246 => 1, // PPD 0246 SWEP26 is bugged and should have a node count of 7, not 6
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
                    let hbx_name = get_file_name(ATARI_BIN, file_index);
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
        let model_name = get_file_name(MODEL_BIN, file_index);
        let file_bytes = bin_model.file_as_bytes(file_index)?;
        models.push(match XBO::import(&file_bytes[..], GAME_SCALE) {
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

                if !model_name.ends_with(".shadow") {
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
                    }

                    // Set surface effects for sequence
                    let mech_surface_effect_idx = file_index / 8;
                    if mech_surface_effect_idx < mech_surface_effect_paths.len() {
                        xbo.set_surface_effects_path(
                            &mech_surface_effect_paths[mech_surface_effect_idx],
                        );
                    }
                }

                let mut node_metas: BTreeMap<usize, Value> = BTreeMap::new();
                if file_index >= 88 && file_index <= 95 {
                    // M2 Chassis
                    for i in [11, 19, 7, 9, 16, 27] {
                        node_metas.insert(i, json!({"armor": true}));
                    }
                    for i in [29, 37] {
                        node_metas.insert(i, json!({"armor": true, "muzzle": true}));
                    }
                    node_metas.insert(36, json!({"recoil": [0.0, 0.0, -5.0]}));
                } else if file_index >= 368 && file_index <= 375 {
                    // M2 Hatch
                    node_metas.insert(3, json!({"armor": true}));
                } else if file_index >= 96 && file_index <= 103 {
                    // M3 Chassis
                    for i in [9, 16] {
                        node_metas.insert(i, json!({"muzzle": true}));
                    }
                }

                // Export GLBIN
                build_path.push(format!("{}.glbin", model_name));
                xbo.write_to_glbin(&build_path)?;
                build_path.pop();

                // Export GLTF
                build_path.push(format!("{}.gltf", model_name));
                xbo.write_to_gltf_with_node_meta(&build_path, node_metas)?;
                build_path.pop();

                Some(xbo)
            }
        });
    }

    /*** CONVERT MENU MODEL FILES ***/
    game_path.push(VTMODEL_BIN);
    let mut bin_vtmodel = BIN::<File>::open(&game_path)?;
    game_path.pop();

    let mut menu_models: Vec<Option<XBO>> = Vec::with_capacity(6);
    for file_index in 758..764 {
        let file_bytes = bin_vtmodel.file_as_bytes(file_index)?;
        menu_models.push(match XBO::import(&file_bytes[..], GAME_SCALE) {
            Err(e) => {
                eprintln!("Failed to convert file {:04}, got error: {}", file_index, e);
                None
            }
            Ok(mut xbo) => {
                let model_name = get_file_name(VTMODEL_BIN, file_index);
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
        });
    }

    build_path.pop(); // Exit models directory

    game_path.pop(); // Exit bin/ directory to media/

    build_path.push("fonts");
    let _ = create_dir(&build_path);
    {
        let uv_font = &efp.uv2_list[3];

        let glyph_advances: Vec<[u8; 199]> = {
            let mut glyph_widths_u16: Vec<u16> = vec![0; 6 * 199];
            xbe.seek_section_offset(".data", 0x31800)?;
            xbe.reader
                .read_u16_into::<LittleEndian>(&mut glyph_widths_u16)?;

            let glyph_widths_u8: Vec<u8> = glyph_widths_u16.iter().map(|&v| v as u8 + 1).collect();

            let (glyph_widths, []) = glyph_widths_u8.as_chunks::<199>() else {
                panic!("glyph_widths length not a multiple of 199");
            };

            assert!(glyph_widths[1] == glyph_widths[3]);
            assert!(glyph_widths[0] == glyph_widths[4]);
            assert!(glyph_widths[1] == glyph_widths[5]);

            glyph_widths[0..3].to_owned()
        };

        const CHAR_COUNT: usize = 96;

        let info_base = Info {
            face: String::new(),
            size: 16,
            bold: false,
            italic: false,
            charset: Charset::Tagged(ANSI),
            unicode: false,
            stretch_h: 100,
            smooth: false,
            aa: 1,
            padding: Padding::default(),
            spacing: Spacing::default(),
            outline: 0,
        };

        let common_base = Common {
            line_height: 16,
            base: 0,
            scale_w: 1,
            scale_h: 1,
            pages: 1,
            packed: false,
            alpha_chnl: bmfont_rs::Packing::Glyph,
            red_chnl: bmfont_rs::Packing::Glyph,
            green_chnl: bmfont_rs::Packing::Glyph,
            blue_chnl: bmfont_rs::Packing::Glyph,
        };

        // OS Gen fonts
        for g in 0..3 {
            let texture_idx = 126 + g;
            let texture = textures[texture_idx].as_ref().unwrap();

            let mut texture_path = build_path.clone();
            texture_path.pop();
            texture_path.push("textures");
            texture_path.push(get_file_name(TEXTURE_BIN, texture_idx));
            texture_path.set_extension("dds");

            let texture_rel_path = diff_paths(&texture_path, &build_path)
                .expect("Unable to get relative texture path for font");

            let mut chars: Vec<Char> = Vec::with_capacity(CHAR_COUNT);
            for i in 0..CHAR_COUNT {
                let frame = &uv_font.frames[i];

                let texture_size = Vec2::new(texture.width as f32, texture.height as f32);

                let start = frame.start * texture_size;
                let size = frame.end * texture_size - start;

                chars.push(Char {
                    id: i as u32 + 32,
                    x: start.x as u16,
                    y: start.y as u16,
                    width: size.x as u16,
                    height: size.y as u16,
                    xoffset: 0,
                    yoffset: 0,
                    //xadvance: (glyph_advances[g][i] as f32 * 0.8) as i16,
                    xadvance: glyph_advances[g][i] as i16,
                    page: 0,
                    chnl: Chnl::ALL,
                });
            }

            let mut info = info_base.clone();

            info.face = format!("OS Gen {}", g + 1);

            let font = Font {
                info,
                common: common_base,
                pages: vec![texture_rel_path.to_str().unwrap().to_owned()],
                chars: chars,
                kernings: Vec::new(),
            };

            println!("Generating font: {}", font.info.face);

            build_path.push(format!("gen{}.fnt", g + 1));
            let file = File::create(&build_path)?;
            build_path.pop();

            let mut writer = BufWriter::new(file);
            bmfont_rs::binary::to_writer(&mut writer, &font)?;
        }

        // OS Common font
        {
            let texture_idx = 126;
            let texture = textures[texture_idx].as_ref().unwrap();

            let mut texture_path = build_path.clone();
            texture_path.pop();
            texture_path.push("textures");
            texture_path.push(get_file_name(TEXTURE_BIN, texture_idx));
            texture_path.set_extension("dds");

            let texture_rel_path = diff_paths(&texture_path, &build_path)
                .expect("Unable to get relative texture path for font");

            let mut chars: Vec<Char> = Vec::with_capacity(CHAR_COUNT);
            for i in 0..CHAR_COUNT {
                let frame = &uv_font.frames[CHAR_COUNT + i];

                let texture_size = Vec2::new(texture.width as f32, texture.height as f32);

                let start = frame.start * texture_size;
                let size = frame.end * texture_size - start;

                chars.push(Char {
                    id: i as u32 + 32,
                    x: start.x as u16,
                    y: start.y as u16,
                    width: size.x as u16,
                    height: size.y as u16,
                    xoffset: 0,
                    yoffset: 0,
                    //xadvance: (glyph_advances[0][i] as f32 * 0.75) as i16,
                    xadvance: glyph_advances[0][i] as i16,
                    page: 0,
                    chnl: Chnl::ALL,
                });
            }

            let mut info = info_base.clone();

            info.face = String::from("OS Common");

            let font = Font {
                info,
                common: common_base,
                pages: vec![texture_rel_path.to_str().unwrap().to_owned()],
                chars: chars,
                kernings: Vec::new(),
            };

            println!("Generating font: {}", font.info.face);

            build_path.push("common.fnt");
            let file = File::create(&build_path)?;
            build_path.pop();

            let mut writer = BufWriter::new(file);
            bmfont_rs::binary::to_writer(&mut writer, &font)?;
        }

        // Debug Font
        {
            let texture_idx = 105;
            let texture = textures[texture_idx].as_ref().unwrap();

            let mut texture_path = build_path.clone();
            texture_path.pop();
            texture_path.push("textures");
            texture_path.push(get_file_name(TEXTURE_BIN, texture_idx));
            texture_path.set_extension("dds");

            let texture_rel_path = diff_paths(&texture_path, &build_path)
                .expect("Unable to get relative texture path for font");

            let mut chars: Vec<Char> = Vec::with_capacity(CHAR_COUNT);
            for i in 0..CHAR_COUNT {
                chars.push(Char {
                    id: i as u32 + 32,
                    x: (i * 8) as u16,
                    y: 0,
                    width: 8,
                    height: texture.height as u16,
                    xoffset: 0,
                    yoffset: 0,
                    //xadvance: (glyph_advances[0][i] as f32 * 0.75) as i16,
                    xadvance: 9,
                    page: 0,
                    chnl: Chnl::ALL,
                });
            }

            let font = Font {
                info: Info {
                    face: String::from("Debug"),
                    size: 8,
                    bold: false,
                    italic: false,
                    charset: Charset::Tagged(ANSI),
                    unicode: false,
                    stretch_h: 100,
                    smooth: false,
                    aa: 1,
                    padding: Padding::default(),
                    spacing: Spacing::default(),
                    outline: 0,
                },
                common: Common {
                    line_height: 8,
                    base: 0,
                    scale_w: 1,
                    scale_h: 1,
                    pages: 1,
                    packed: false,
                    alpha_chnl: bmfont_rs::Packing::Glyph,
                    red_chnl: bmfont_rs::Packing::Glyph,
                    green_chnl: bmfont_rs::Packing::Glyph,
                    blue_chnl: bmfont_rs::Packing::Glyph,
                },
                pages: vec![texture_rel_path.to_str().unwrap().to_owned()],
                chars: chars,
                kernings: Vec::new(),
            };

            println!("Generating font: {}", font.info.face);

            build_path.push("debug.fnt");
            let file = File::create(&build_path)?;
            build_path.pop();

            let mut writer = BufWriter::new(file);
            bmfont_rs::binary::to_writer(&mut writer, &font)?;
        }

        // Display Fonts
        for c in 0..6 {
            let texture_idx = 145 + c * 2;

            let mut texture_path = build_path.clone();
            texture_path.pop();
            texture_path.push("textures");
            texture_path.push(get_file_name(TEXTURE_BIN, texture_idx));
            texture_path.set_extension("dds");

            let texture_rel_path = diff_paths(&texture_path, &build_path)
                .expect("Unable to get relative texture path for font");

            let mut chars: Vec<Char> = Vec::with_capacity(100);
            for xi in 0..10u16 {
                for yi in 0..10u16 {
                    let id = (xi * 10 + yi + 32) as u32;
                    let width = if DISPLAY_FONT_HALF_WIDTH_CHARS.contains(
                        &char::from_u32(id).expect("Invalid character ID in Display font"),
                    ) {
                        8
                    } else {
                        16
                    };

                    chars.push(Char {
                        id,
                        x: xi * 16,
                        y: yi * 24,
                        width,
                        height: 24,
                        xoffset: 0,
                        yoffset: 0,
                        //xadvance: (glyph_advances[0][i] as f32 * 0.75) as i16,
                        xadvance: width as i16, // Seamless font
                        page: 0,
                        chnl: Chnl::ALL,
                    });
                }
            }

            let font = Font {
                info: Info {
                    face: format!("Display {}", c),
                    size: 24,
                    bold: false,
                    italic: false,
                    charset: Charset::Tagged(ANSI),
                    unicode: false,
                    stretch_h: 100,
                    smooth: false,
                    aa: 1,
                    padding: Padding::default(),
                    spacing: Spacing::default(),
                    outline: 0,
                },
                common: Common {
                    line_height: 24,
                    base: 0,
                    scale_w: 1,
                    scale_h: 1,
                    pages: 1,
                    packed: false,
                    alpha_chnl: bmfont_rs::Packing::Glyph,
                    red_chnl: bmfont_rs::Packing::Glyph,
                    green_chnl: bmfont_rs::Packing::Glyph,
                    blue_chnl: bmfont_rs::Packing::Glyph,
                },
                pages: vec![texture_rel_path.to_str().unwrap().to_owned()],
                chars: chars,
                kernings: Vec::new(),
            };

            println!("Generating font: {}", font.info.face);

            build_path.push(format!("display{}.fnt", c));
            let file = File::create(&build_path)?;
            build_path.pop();

            let mut writer = BufWriter::new(file);
            bmfont_rs::binary::to_writer(&mut writer, &font)?;
        }
    }
    build_path.pop(); // Exit fonts directory

    return Ok(());
}
