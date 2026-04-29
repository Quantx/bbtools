use std::f32::consts::PI;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::{array, cmp};

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};

use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD
use glam::{U8Vec4, Vec2, Vec4Swizzles};

use crate::bbtools::bin::BIN;
use crate::bbtools::eff::EffectPosition;
use crate::bbtools::obj::{ModelConfig, ModelConfigFlags};
use crate::bbtools::obj::{POS_ROT_SIZE, PosRot};
use crate::bbtools::x86::X86Context;
use crate::bbtools::xbe::XBE;

use crate::titles::FnIndexToPath;

use crate::bbtools::write_godot_path;

const LIGHT_SIZE: usize = 16;
#[derive(Clone, Copy, Default)]
pub struct Light {
    color: Vec3, // RGB
    energy: f32,
}

impl Light {
    pub fn import(buf: &[u8], scale: f32) -> Self {
        assert!(buf.len() == LIGHT_SIZE);
        return Light {
            color: Vec3::new(
                LittleEndian::read_f32(&buf[0..4]),
                LittleEndian::read_f32(&buf[4..8]),
                LittleEndian::read_f32(&buf[8..12]),
            ) * scale,
            energy: LittleEndian::read_f32(&buf[12..16]),
        };
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        for c in self.color.to_array() {
            writer.write_f32::<LittleEndian>(c)?;
        }
        writer.write_f32::<LittleEndian>(self.energy)?;
        return Ok(());
    }
}

impl fmt::Display for Light {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Color: {:.03}, Energy: {:.03}", self.color, self.energy)
    }
}

const POINT_LIGHT_SIZE: usize = 32;
#[derive(Clone, Copy, Default)]
pub struct PointLight {
    //unknown: u32,
    position: Vec3,
    light: Light,
}

impl PointLight {
    pub fn import(buf: &[u8], scale: f32, cockpit_position: Vec3) -> Self {
        assert!(buf.len() == POINT_LIGHT_SIZE);
        /*
        let unknown = LittleEndian::read_u32(&buf[0..4]);
        if unknown != 0 && unknown != u16::MAX as u32 && unknown != u32::MAX {
            println!("Valid unknown hex {:08X}", unknown);
            println!("Valid unknown float {}", LittleEndian::read_f32(&buf[0..4]));
        }
        */
        return PointLight {
            position: Vec3::new(
                LittleEndian::read_f32(&buf[4..8]),
                LittleEndian::read_f32(&buf[8..12]),
                LittleEndian::read_f32(&buf[12..16]),
            ) * scale
                - cockpit_position,
            light: Light::import(&buf[16..32], scale),
        };
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.position.x)?;
        writer.write_f32::<LittleEndian>(self.position.y)?;
        writer.write_f32::<LittleEndian>(self.position.z)?;
        self.light.write(writer)?;
        return Ok(());
    }
}

impl fmt::Display for PointLight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Position: {:.02}, {}", self.position, self.light)
    }
}

const SCENE_LIGHT_SIZE: usize = 40;
#[derive(Clone, Copy, Default)]
pub struct SceneLight {
    ambient_light: Light,
    sun_light: Light,
    sun_yaw: f32,
    sun_pitch: f32,
}

impl SceneLight {
    pub fn import(buf: &[u8]) -> Self {
        assert!(buf.len() == SCENE_LIGHT_SIZE);
        return SceneLight {
            ambient_light: Light::import(&buf[0..16], 1.0),
            sun_light: Light::import(&buf[16..32], 1.0),
            sun_yaw: LittleEndian::read_f32(&buf[32..36]),
            sun_pitch: LittleEndian::read_f32(&buf[36..40]),
        };
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        self.ambient_light.write(writer)?;
        self.sun_light.write(writer)?;
        writer.write_f32::<LittleEndian>(self.sun_yaw)?;
        writer.write_f32::<LittleEndian>(self.sun_pitch)?;
        return Ok(());
    }
}

impl fmt::Display for SceneLight {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Ambient - {} | Sun - Yaw: {:.03}, Pitch: {:.03}, {}",
            self.ambient_light, self.sun_yaw, self.sun_pitch, self.sun_light
        )
    }
}

const POINT_LIGHT_FRAME_SIZE: usize = POINT_LIGHT_SIZE + 4; // 44
#[derive(Clone, Copy, Default)]
pub struct PointLightFrame(f32, PointLight);

const SCENE_LIGHT_FRAME_SIZE: usize = SCENE_LIGHT_SIZE + 4; // 36
#[derive(Clone, Copy, Default)]
pub struct SceneLightFrame(f32, SceneLight);

#[derive(Clone, Default)]
pub struct LightingAnimation {
    scene_frames: Vec<SceneLightFrame>,
    point_frames: Vec<Vec<PointLightFrame>>,
}

impl LightingAnimation {
    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(self.scene_frames.len() as u32)?;
        for sf in self.scene_frames.iter() {
            writer.write_f32::<LittleEndian>(sf.0)?;
            sf.1.write(writer)?;
        }

        writer.write_u32::<LittleEndian>(self.point_frames.len() as u32)?;
        for pfl in self.point_frames.iter() {
            writer.write_u32::<LittleEndian>(pfl.len() as u32)?;
            for pf in pfl.iter() {
                writer.write_f32::<LittleEndian>(pf.0)?;
                pf.1.write(writer)?;
            }
        }

        return Ok(());
    }
}

impl fmt::Display for LightingAnimation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Scene Lighting - Frames: {:02}", self.scene_frames.len())?;
        for (i, sf) in self.scene_frames.iter().enumerate() {
            writeln!(f, "  Frame {:02} - Time {:.02} | {}", i, sf.0, sf.1)?;
        }
        for (i, pfl) in self.point_frames.iter().enumerate() {
            writeln!(f, "Point Light {} - Frames: {:02}", i, pfl.len())?;
            for (j, pf) in pfl.iter().enumerate() {
                writeln!(f, "  Frame {:02} - Time {:.02} | {}", j, pf.0, pf.1)?;
            }
        }
        return Ok(());
    }
}

pub struct COC {
    pub idx: usize,
    lights: Vec<PointLight>,
}

impl COC {
    pub fn import(buf: &[u8], idx: usize, scale: f32, cockpit_position: Vec3) -> Self {
        let light_count = LittleEndian::read_u32(&buf[0..4]) as usize;

        let mut light_ptr = &buf[4..];

        let mut lights: Vec<PointLight> = Vec::with_capacity(light_count);
        for _ in 0..light_count {
            let light =
                PointLight::import(&light_ptr[0..POINT_LIGHT_SIZE], scale, cockpit_position);
            lights.push(light);
            light_ptr = &light_ptr[POINT_LIGHT_SIZE..];
        }

        return COC { idx, lights };
    }

    pub fn import_bin(
        path: &Path,
        scale: f32,
        cockpit_position: Vec3,
    ) -> Result<Vec<Self>, std::io::Error> {
        let file = File::open(path)?;
        let mut coc_bin = BIN::from_reader(BufReader::new(file))?;

        println!("Importing {} COC files", coc_bin.file_count());

        let mut coc_list: Vec<COC> = Vec::with_capacity(coc_bin.file_count());
        for file_index in 0..coc_bin.file_count() {
            let file_bytes = coc_bin.file_as_bytes(file_index)?;
            let coc = COC::import(file_bytes.as_slice(), file_index, scale, cockpit_position);
            coc_list.push(coc);
        }

        return Ok(coc_list);
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(self.lights.len() as u32)?;
        for light in self.lights.iter() {
            light.write(writer)?;
        }

        return Ok(());
    }
}

impl fmt::Display for COC {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "COC {:02} - Scene Lights: {}",
            self.idx,
            self.lights.len()
        )?;

        for (i, light) in self.lights.iter().enumerate() {
            writeln!(f, "  Light {} | {}", i, light)?;
        }

        return Ok(());
    }
}

pub struct AMB {
    pub idx: usize,
    light: SceneLight,
}

impl AMB {
    pub fn import(buf: &[u8], idx: usize) -> Self {
        let light = SceneLight::import(&buf[0..SCENE_LIGHT_SIZE]);
        return AMB { idx, light };
    }

    pub fn import_bin(path: &Path) -> Result<Vec<Self>, std::io::Error> {
        let file = File::open(path)?;
        let mut amb_bin = BIN::from_reader(BufReader::new(file))?;

        println!("Importing {} AMB files", amb_bin.file_count());

        let mut amb_list: Vec<AMB> = Vec::with_capacity(amb_bin.file_count());
        for file_index in 0..amb_bin.file_count() {
            let file_bytes = amb_bin.file_as_bytes(file_index)?;
            let amb = AMB::import(file_bytes.as_slice(), file_index);
            amb_list.push(amb);
        }

        return Ok(amb_list);
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        self.light.write(writer)
    }
}

impl fmt::Display for AMB {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AMB {:02} | {}", self.idx, self.light)
    }
}

// ouch
const CBT_FRAME_COUNT: usize = 50;
const CBT_POINT_LIGHT_COUNT: usize = 10;
pub struct CBT {
    pub idx: usize,
    animation: LightingAnimation,
}

impl CBT {
    pub fn import(buf: &[u8], idx: usize, scale: f32, cockpit_offset: Vec3, fps: f32) -> Self {
        let spf = 1.0 / fps;
        let mut light_ptr = &buf[..];

        //println!("Importing up to {} scene light frames", CBT_FRAME_COUNT);

        let mut scene_frames_done = false;
        let mut scene_frames: Vec<SceneLightFrame> = Vec::with_capacity(CBT_FRAME_COUNT);
        for _ in 0..CBT_FRAME_COUNT {
            let frame = LittleEndian::read_u32(&light_ptr[0..4]);
            let scene_light = SceneLight::import(&light_ptr[4..SCENE_LIGHT_FRAME_SIZE]);

            if frame == u32::MAX {
                scene_frames_done = true;
            }

            if !scene_frames_done {
                scene_frames.push(SceneLightFrame(frame as f32 * spf, scene_light));
            }

            light_ptr = &light_ptr[SCENE_LIGHT_FRAME_SIZE..];
        }

        let mut point_frames: Vec<Vec<PointLightFrame>> = Vec::with_capacity(CBT_POINT_LIGHT_COUNT);
        for i in 0..CBT_POINT_LIGHT_COUNT {
            point_frames.push(Vec::new()); // Don't actually allocate any space, since there's only ever a handful of frames

            let mut point_frames_done = false;
            for _ in 0..CBT_FRAME_COUNT {
                let frame = LittleEndian::read_u32(&light_ptr[0..4]);
                let point_light = PointLight::import(
                    &light_ptr[4..POINT_LIGHT_FRAME_SIZE],
                    scale,
                    cockpit_offset,
                );

                if frame == u32::MAX
                    // Handle some edge cases where the data is corrupt
                    || point_light.position.is_nan()
                    || point_light.position.min_element() < -1000.0
                {
                    point_frames_done = true;
                }

                if !point_frames_done {
                    point_frames[i].push(PointLightFrame(frame as f32 * spf, point_light));
                }

                light_ptr = &light_ptr[POINT_LIGHT_FRAME_SIZE..];
            }
        }

        point_frames.retain(|pf| !pf.is_empty());

        return CBT {
            idx,
            animation: LightingAnimation {
                scene_frames,
                point_frames,
            },
        };
    }

    pub fn import_bin(
        path: &Path,
        scale: f32,
        cockpit_offset: Vec3,
        fps: f32,
    ) -> Result<Vec<Self>, std::io::Error> {
        let file = File::open(path)?;
        let mut cbt_bin = BIN::from_reader(BufReader::new(file))?;

        println!("Importing {} CBT files", cbt_bin.file_count());

        let mut cbt_list: Vec<Self> = Vec::with_capacity(cbt_bin.file_count());
        for file_index in 0..cbt_bin.file_count() {
            if cbt_bin.file_length(file_index) == 40000 {
                continue;
            }

            let file_bytes = cbt_bin.file_as_bytes(file_index)?;
            let cbt = CBT::import(
                file_bytes.as_slice(),
                file_index,
                scale,
                cockpit_offset,
                fps,
            );

            cbt_list.push(cbt);
        }

        return Ok(cbt_list);
    }

    pub fn find_by_idx(cbt_list: &[CBT], idx: usize) -> Option<&Self> {
        cbt_list.iter().find(|cbt| cbt.idx == idx)
    }
}

impl fmt::Display for CBT {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CBT {:02} - {}", self.idx, self.animation)
    }
}

#[derive(Default)]
pub struct CockpitLighting {
    point_light_max: usize,
    pub cockpit_closed_time: f32,
    animations: Vec<LightingAnimation>,
}

impl CockpitLighting {
    pub fn add_animation(&mut self, animation: LightingAnimation) {
        self.point_light_max = cmp::max(self.point_light_max, animation.point_frames.len());
        self.animations.push(animation);
    }

    pub fn add_cbt(&mut self, cbt: &CBT) {
        self.add_animation(cbt.animation.clone())
    }

    pub fn add_amb_coc(&mut self, amb: &AMB, coc: &COC) {
        self.add_animation(LightingAnimation {
            scene_frames: vec![SceneLightFrame(0.0, amb.light)],
            point_frames: coc
                .lights
                .iter()
                .map(|l| vec![PointLightFrame(0.0, l.clone())])
                .collect(),
        })
    }

    pub fn export(&self, path: &Path) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writer.write_u32::<LittleEndian>(self.point_light_max as u32)?;
        writer.write_f32::<LittleEndian>(self.cockpit_closed_time)?;
        writer.write_u32::<LittleEndian>(self.animations.len() as u32)?;
        for animation in self.animations.iter() {
            animation.write(&mut writer)?;
        }
        return Ok(());
    }
}

const DIAL_COUNT: usize = 6;

#[derive(Clone, Copy, Default)]
pub struct DialsConfig {
    model: u8,
    max_speed_kmh: u8,
    bones: [u8; DIAL_COUNT],
    scale: [f32; DIAL_COUNT],
}

impl DialsConfig {
    pub fn import(xbe: &mut XBE) -> Result<[Self; COCKPIT_COUNT], std::io::Error> {
        xbe.seek_section_offset(".text", 0x63E80)?;
        let mut x86_ctx = X86Context::new(0);

        let mut bytes_executed: usize = 0;
        while bytes_executed < 524 {
            bytes_executed += x86_ctx.execute_instruction(xbe)?;
        }

        xbe.reader.seek_relative(22)?;

        bytes_executed = 0;
        while bytes_executed < 48 {
            bytes_executed += x86_ctx.execute_instruction(xbe)?;
        }

        xbe.reader.seek_relative(4)?;

        bytes_executed = 0;
        while bytes_executed < 35 {
            bytes_executed += x86_ctx.execute_instruction(xbe)?;
        }

        xbe.reader.seek_relative(2)?;

        bytes_executed = 0;
        while bytes_executed < 19 {
            bytes_executed += x86_ctx.execute_instruction(xbe)?;
        }

        let stack = x86_ctx.get_stack();
        let mut stack_ptr = &stack[12..];

        let mut dials: [Self; COCKPIT_COUNT] = [Self::default(); _];
        for i in 0..COCKPIT_COUNT {
            dials[i].model = LittleEndian::read_u32(&stack_ptr[0..4]) as u8;
            stack_ptr = &stack_ptr[4..];
        }

        for i in 0..COCKPIT_COUNT {
            let max_speed_kmh = LittleEndian::read_f32(&stack_ptr[0..4]);
            stack_ptr = &stack_ptr[4..];

            assert!(max_speed_kmh >= 0.0 && max_speed_kmh <= u8::MAX as f32);
            dials[i].max_speed_kmh = max_speed_kmh as u8;
        }

        for i in 0..COCKPIT_COUNT {
            let mut scale: [u32; DIAL_COUNT] = [0; _];
            LittleEndian::read_u32_into(&stack_ptr[0..24], &mut scale);
            stack_ptr = &stack_ptr[24..];

            dials[i].scale = scale.map(|v| {
                if v == u32::MAX {
                    0.0
                } else {
                    (v as f32).to_radians()
                }
            });
        }

        for i in 0..COCKPIT_COUNT {
            let mut bones: [u32; DIAL_COUNT] = [0; _];
            LittleEndian::read_u32_into(&stack_ptr[0..24], &mut bones);
            stack_ptr = &stack_ptr[24..];

            dials[i].bones = bones.map(|v| v as u8);
        }

        return Ok(dials);
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u8(self.model)?;
        writer.write_u8(self.max_speed_kmh)?;
        for i in 0..DIAL_COUNT {
            writer.write_u8(self.bones[i])?;
            writer.write_f32::<LittleEndian>(self.scale[i])?;
        }

        return Ok(());
    }
}

const COCKPIT_COUNT: usize = 6;
const INDICATOR_LIGHT_COUNT: usize = 9;
const COMM_LIGHT_COUNT: usize = 5;
const PILOT_POSE_COUNT: usize = 7;
pub struct Cockpit {
    id: u8,

    offset: Vec3,
    pilot_poses: [PosRot; 7],

    display_textures: [u16; 2],
    chassis_textures: [u16; 2],

    indicators_lights: [Option<EffectPosition>; INDICATOR_LIGHT_COUNT],
    eject_light: EffectPosition,
    comm_lights: Option<[EffectPosition; COMM_LIGHT_COUNT]>,

    multi_monitors: (u8, u8),
    mweps: (u8, u8),
    sweps: (u8, u8),
    tuner: (u8, u8),
    comms: (u8, u8),

    dials_config: DialsConfig,

    monitor_models: Vec<ModelConfigFlags>,
    chassis_models: Vec<ModelConfigFlags>,
    display_models: Vec<ModelConfigFlags>,
}

impl Cockpit {
    pub fn import(
        xbe: &mut XBE,
        scale: f32,
        cockpit_position: Vec3,
    ) -> Result<Vec<Self>, std::io::Error> {
        let indicator_lights = EffectPosition::import_separate(
            xbe,
            ".data",
            0x36F78,
            ".data",
            0x37050,
            COCKPIT_COUNT * INDICATOR_LIGHT_COUNT,
            scale,
        )?;

        let mut dials = DialsConfig::import(xbe)?;

        xbe.seek_section_offset("seg00", 12)?;
        let offset = Vec3::new(
            xbe.reader.read_f32::<LittleEndian>()?,
            xbe.reader.read_f32::<LittleEndian>()?,
            xbe.reader.read_f32::<LittleEndian>()?,
        ) * scale
            - cockpit_position;

        let mut pilot_poses: [[PosRot; 7]; 6] = [[PosRot::default(); 7]; 6];
        xbe.seek_section_offset(".data", 0x105D0)?;
        for c in 0..COCKPIT_COUNT {
            for p in 0..PILOT_POSE_COUNT {
                let mut buf = vec![0; POS_ROT_SIZE];
                xbe.reader.read_exact(&mut buf)?;
                let mut pose = PosRot::import(buf.as_slice());
                pose.0 *= scale;
                pose.0 -= cockpit_position;

                let rot_y = pose.1.x;
                pose.1.x = pose.1.y;
                pose.1.y = rot_y - PI;

                pilot_poses[c][p] = pose;
            }
        }

        let (cockpit_indicator_lights, []) = indicator_lights.as_chunks::<INDICATOR_LIGHT_COUNT>()
        else {
            panic!("indicator_lights length not a multiple of INDICATOR_LIGHT_COUNT");
        };

        let mut cockpit_eject_lights: Vec<EffectPosition> = Vec::with_capacity(COCKPIT_COUNT);
        {
            xbe.seek_section_offset(".text", 0x5C6EE)?;
            let mut x86_ctx = X86Context::new(0x4C);

            let mut bytes_executed: usize = 0;
            while bytes_executed < 144 {
                bytes_executed += x86_ctx.execute_instruction(xbe)?;
            }

            let stack = x86_ctx.get_stack();
            let mut stack_ptr = &stack[4..];

            for i in 0..COCKPIT_COUNT {
                cockpit_eject_lights.push(EffectPosition {
                    idx: i as u16 + 273,
                    position: Vec3::new(
                        LittleEndian::read_f32(&stack_ptr[0..4]),
                        LittleEndian::read_f32(&stack_ptr[4..8]),
                        LittleEndian::read_f32(&stack_ptr[8..12]),
                    ) * scale
                        - cockpit_position,
                });
                stack_ptr = &stack_ptr[12..];
            }
        }

        let mut comm_lights: [EffectPosition; COMM_LIGHT_COUNT] = [EffectPosition {
            idx: 289,
            position: Vec3::ZERO,
        }; _];
        {
            let mut x86_ctx = X86Context::new(0x64);
            xbe.seek_section_offset(".text", 0x6A2FC)?;

            let mut bytes_executed: usize = 0;
            while bytes_executed < 120 {
                bytes_executed += x86_ctx.execute_instruction(xbe)?;
            }

            let stack = x86_ctx.get_stack();
            let mut stack_ptr = &stack[0x28..];

            for i in 0..COMM_LIGHT_COUNT {
                comm_lights[i].position = Vec3::new(
                    LittleEndian::read_f32(&stack_ptr[0..4]),
                    LittleEndian::read_f32(&stack_ptr[4..8]),
                    LittleEndian::read_f32(&stack_ptr[8..12]),
                ) * scale
                    - cockpit_position;
                stack_ptr = &stack_ptr[12..];
            }
        };

        /*** Assemble Cockpit Model Configs ***/
        let mut cockpit_model_configs;
        {
            let mut x86_ctx = X86Context::new(64);
            xbe.seek_section_offset(".text", 0x59E80)?;

            let mut bytes_executed: usize = 0;
            while bytes_executed < 3391 {
                bytes_executed += x86_ctx.execute_instruction(xbe)?;
            }

            xbe.reader.seek_relative(7)?; // goto: .text + 0x5ABC6
            bytes_executed += x86_ctx.execute_instruction(xbe)?;

            xbe.reader.seek_relative(37)?; // goto: .text + 0x5ABF3
            bytes_executed += x86_ctx.execute_instruction(xbe)?;

            xbe.reader.seek_relative(4)?; // goto: .text + 0x5ABFF
            while bytes_executed < 3538 {
                bytes_executed += x86_ctx.execute_instruction(xbe)?;
            }
            assert!(xbe.reader.read_u16::<LittleEndian>()? == 0x0B7E); // Safety check to ensure all data was processed

            let stack = x86_ctx.get_stack();

            cockpit_model_configs = [
                (
                    // Cockpit 0
                    ModelConfigFlags::import_multiple_a(&stack[0xA4..], 4),
                    ModelConfigFlags::import_multiple_a(&stack[0x260..], 12),
                    Vec::<ModelConfigFlags>::new(),
                    8u8,              // Display split
                    (7u8, 0u8),       // Chassis
                    (5u8, 9u8),       // MWEP
                    (6u8, 10u8),      // SWEP
                    (8u8, 7u8),       // Tuner, Tuner bone
                    (0xFFu8, 0xFFu8), // Comms (Chassis and Display)
                ),
                (
                    // Cockpit 1
                    ModelConfigFlags::import_multiple_a(&stack[0x14..], 4),
                    ModelConfigFlags::import_multiple_a(&stack[0x38C..], 14),
                    Vec::<ModelConfigFlags>::new(),
                    11u8,
                    (0xFFu8, 0u8),
                    (5u8, 12u8),
                    (6u8, 13u8),
                    (8u8, 0xFFu8),
                    (7u8, 10u8), // Comms (Chassis and Display)
                ),
                (
                    // Cockpit 2
                    ModelConfigFlags::import_multiple_a(&stack[0x104..], 5),
                    ModelConfigFlags::import_multiple_a(&stack[0x2F0..], 13),
                    Vec::<ModelConfigFlags>::new(),
                    7u8,
                    (0xFFu8, 5u8),
                    (0xFFu8, 10u8),
                    (0xFFu8, 11u8),
                    (9u8, 1u8),
                    (0xFFu8, 0xFFu8), // Comms (Chassis and Display)
                ),
                (
                    // Cockpit 3
                    ModelConfigFlags::import_multiple_a(&stack[0xD4..], 4),
                    ModelConfigFlags::import_multiple_a(&stack[0x140..], 12),
                    Vec::<ModelConfigFlags>::new(),
                    8u8,
                    (0xFFu8, 0u8),
                    (5u8, 9u8),
                    (6u8, 10u8),
                    (8u8, 5u8),
                    (0xFFu8, 0xFFu8), // Comms (Chassis and Display)
                ),
                (
                    // Cockpit 4
                    ModelConfigFlags::import_multiple_a(&stack[0x74..], 4),
                    ModelConfigFlags::import_multiple_a(&stack[0x1D0..], 12),
                    Vec::<ModelConfigFlags>::new(),
                    8u8,
                    (0xFFu8, 0u8),
                    (5u8, 9u8),
                    (6u8, 10u8),
                    (8u8, 7u8),
                    (0xFFu8, 0xFFu8), // Comms (Chassis and Display)
                ),
                (
                    // Cockpit 5
                    ModelConfigFlags::import_multiple_a(&stack[0x44..], 4),
                    ModelConfigFlags::import_multiple_a(&stack[0x434..], 14),
                    Vec::<ModelConfigFlags>::new(),
                    8u8,
                    (0xFFu8, 0u8),
                    (5u8, 8u8),
                    (6u8, 9u8),
                    (11u8, 1u8),
                    (0xFFu8, 0xFFu8), // Comms (Chassis and Display)
                ),
            ];

            {
                // Fixup cockpit 1
                cockpit_model_configs[1].1.insert(
                    10,
                    ModelConfigFlags {
                        modcfg: ModelConfig {
                            model: 1268,
                            animation: Some(121),
                            sequence: None,
                            hitbox: None,
                        },
                        flags: 0,
                    },
                );
                dials[1].model += 1;
            }

            {
                // Fixup cockpit 2
                cockpit_model_configs[2].1[1] = cockpit_model_configs[2].0.remove(4);

                //let comm_model = cockpit_model_configs[2].1.remove(9);
                //cockpit_model_configs[2].1.push(comm_model);
            }

            for (mmcs, cmcs, dmcs, split_idx, _, _, _, _, _) in cockpit_model_configs.iter_mut() {
                for mmc in mmcs.iter_mut() {
                    // Hitboxes are all imported as Some(0) set them to None instead
                    mmc.modcfg.hitbox = None;
                    if mmc.modcfg.model == 1237 {
                        mmc.modcfg.sequence = None;
                    }

                    //println!("MMC: {}", mmc);
                }

                for cmc in cmcs.iter_mut() {
                    // Hitboxes are all imported as Some(0) set them to None instead
                    cmc.modcfg.hitbox = None;
                    if cmc.modcfg.model == 1237 {
                        cmc.modcfg.sequence = None;
                    }

                    //println!("CMC: {}", cmc);
                }

                dmcs.extend(cmcs.split_off(*split_idx as usize));
            }
        }

        let mut cockpits: Vec<Cockpit> = Vec::with_capacity(COCKPIT_COUNT);
        for i in 0..COCKPIT_COUNT {
            cockpits.push(Cockpit {
                id: i as u8,

                offset,
                pilot_poses: pilot_poses[i],

                chassis_textures: [133 + 2 * i as u16, 134 + 2 * i as u16],
                display_textures: [145 + 2 * i as u16, 146 + 2 * i as u16],

                indicators_lights: cockpit_indicator_lights[i],
                eject_light: cockpit_eject_lights[i],
                comm_lights: if i == 1 { Some(comm_lights) } else { None },
                monitor_models: cockpit_model_configs[i].0.clone(),
                chassis_models: cockpit_model_configs[i].1.clone(),
                display_models: cockpit_model_configs[i].2.clone(),

                multi_monitors: cockpit_model_configs[i].4,
                mweps: cockpit_model_configs[i].5,
                sweps: cockpit_model_configs[i].6,
                tuner: cockpit_model_configs[i].7,
                comms: cockpit_model_configs[i].8,

                dials_config: dials[i],
            });
        }

        return Ok(cockpits);
    }

    pub fn get_model_configs(&self) -> Vec<ModelConfig> {
        let mut mcs: Vec<ModelConfig> = Vec::with_capacity(
            self.monitor_models.len() + self.chassis_models.len() + self.display_models.len(),
        );
        for cmc in self.monitor_models.iter() {
            mcs.push(cmc.modcfg);
        }
        for cmc in self.chassis_models.iter() {
            mcs.push(cmc.modcfg);
        }
        for cmc in self.display_models.iter() {
            mcs.push(cmc.modcfg);
        }
        return mcs;
    }

    pub fn export(
        &self,
        path: &Path,
        texture_path_fn: FnIndexToPath,
        model_path_fn: FnIndexToPath,
        lighting_path_fn: FnIndexToPath,
    ) -> Result<(), std::io::Error> {
        let mut cockpit_path = path.to_owned();

        cockpit_path.push(format!("Cockpit_{}.cockpit_scene", self.id));
        {
            let file = File::create(&cockpit_path)?;
            let mut writer = BufWriter::new(file);

            writer.write_u8(self.id)?;

            writer.write_f32::<LittleEndian>(self.offset.x)?;
            writer.write_f32::<LittleEndian>(self.offset.y)?;
            writer.write_f32::<LittleEndian>(self.offset.z)?;

            writer.write_u8(self.pilot_poses.len() as u8)?;
            for pose in self.pilot_poses {
                pose.write(&mut writer)?;
            }

            for texture_id in self.chassis_textures {
                let texture_path = texture_path_fn(texture_id as usize);
                write_godot_path(&texture_path, &mut writer)?;
            }

            for texture_id in self.display_textures {
                let texture_path = texture_path_fn(texture_id as usize);
                write_godot_path(&texture_path, &mut writer)?;
            }

            let lighting_path = lighting_path_fn(self.id as usize);
            write_godot_path(&lighting_path, &mut writer)?;

            writer.write_u8(self.monitor_models.len() as u8)?;
            for model in self.monitor_models.iter() {
                let model_path = model_path_fn(model.modcfg.model as usize);
                write_godot_path(&model_path, &mut writer)?;
                writer.write_u32::<LittleEndian>(model.flags)?;
            }

            writer.write_u8(self.chassis_models.len() as u8)?;
            for model in self.chassis_models.iter() {
                let model_path = model_path_fn(model.modcfg.model as usize);
                write_godot_path(&model_path, &mut writer)?;
                writer.write_u32::<LittleEndian>(model.flags)?;
            }

            writer.write_u8(self.display_models.len() as u8)?;
            for model in self.display_models.iter() {
                let model_path = model_path_fn(model.modcfg.model as usize);
                write_godot_path(&model_path, &mut writer)?;
                writer.write_u32::<LittleEndian>(model.flags)?;
            }

            for (a, b) in [
                self.multi_monitors,
                self.mweps,
                self.sweps,
                self.tuner,
                self.comms,
            ] {
                writer.write_u8(a)?;
                writer.write_u8(b)?;
            }

            self.dials_config.write(&mut writer)?;

            self.eject_light.write(&mut writer)?;

            for indicator_light in self.indicators_lights.iter() {
                if let Some(light) = indicator_light {
                    light.write(&mut writer)?;
                } else {
                    writer.write_u16::<LittleEndian>(u16::MAX)?;
                }
            }

            if let Some(comm_lights) = self.comm_lights {
                for light in comm_lights {
                    light.write(&mut writer)?;
                }
            } else {
                for _ in 0..COMM_LIGHT_COUNT {
                    writer.write_u16::<LittleEndian>(u16::MAX)?;
                }
            }
        }
        cockpit_path.pop();

        return Ok(());
    }
}

pub enum OSAnimType {
    Start,
    Points { vertices: [Vec2; 4] },
    Rotate { clockwise: bool }, // Unused
    Color { color: U8Vec4 },
    Colors { colors: [U8Vec4; 4] },
    Text,                 // OSDraw::Text only
    Scale { scale: f32 }, // OSDraw::LinesDef only
    Stop,                 // Unused
}

impl OSAnimType {
    pub fn import(command: u32, buf: &[u8]) -> Self {
        match command {
            0x01 => Self::Start,
            0x02 => {
                let mut vertex_components: [f32; 8] = [0.0; _];
                LittleEndian::read_f32_into(&buf[0..32], vertex_components.as_mut_slice());

                Self::Points {
                    vertices: array::from_fn(|i| {
                        Vec2::from_slice(&vertex_components[i * 2..i * 2 + 2])
                    }),
                }
            }
            0x04 => Self::Rotate {
                clockwise: LittleEndian::read_u32(&buf[4..8]) != 0,
            },
            0x08 => Self::Color {
                color: U8Vec4::from_slice(&buf[0..4]).zyxw(),
            },
            0x10 => Self::Colors {
                colors: array::from_fn(|i| U8Vec4::from_slice(&buf[i * 4..i * 4 + 4]).zyxw()),
            },
            0x20 => Self::Text,
            0x40 => Self::Scale {
                scale: LittleEndian::read_f32(&buf[0..4]),
            },
            0x80 => Self::Stop,
            _ => todo!("OSAnimType unknown command"),
        }
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Start => {
                writer.write_u8(0)?;
            }
            Self::Points { vertices } => {
                writer.write_u8(1)?;
                for vertex in vertices.iter() {
                    writer.write_f32::<LittleEndian>(vertex.x)?;
                    writer.write_f32::<LittleEndian>(vertex.y)?;
                }
            }
            Self::Rotate { clockwise } => {
                writer.write_u8(2)?;
                writer.write_u8(*clockwise as u8)?;
            }
            Self::Color { color } => {
                writer.write_u8(3)?;
                writer.write_all(color.to_array().as_slice())?;
            }
            Self::Colors { colors } => {
                writer.write_u8(4)?;
                for color in colors.iter() {
                    writer.write_all(color.to_array().as_slice())?;
                }
            }
            Self::Text => {
                writer.write_u8(5)?;
            }
            Self::Scale { scale } => {
                writer.write_u8(6)?;
                writer.write_f32::<LittleEndian>(*scale)?;
            }
            Self::Stop => {
                writer.write_u8(7)?;
            }
        }
        return Ok(());
    }
}

impl fmt::Display for OSAnimType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Start => write!(f, "Start"),
            Self::Points { vertices } => write!(
                f,
                "Points - V0: {:.02}, V1: {:.02}, V2: {:.02}, V3: {:.02}",
                vertices[0], vertices[1], vertices[2], vertices[3]
            ),
            Self::Rotate { clockwise } => write!(
                f,
                "Rotate - {}",
                if *clockwise {
                    "Clockwise"
                } else {
                    "Counterclockwise"
                }
            ),
            Self::Color { color } => write!(f, "Color - C: {}", color),
            Self::Colors { colors } => write!(
                f,
                "Color - C0: {}, C1: {}, C2: {}, C3: {}",
                colors[0], colors[1], colors[2], colors[3]
            ),
            Self::Text => write!(f, "Text"),
            Self::Scale { scale } => write!(f, "Scale - Factor: {:.02}", scale),
            Self::Stop => write!(f, "Stop"),
        }
    }
}

const OS_ANIM_SIZE: usize = 64;
const OS_ANIM_COUNT: usize = OS_DRAW_COUNT * OS_DRAW_ANIM_COUNT;
pub struct OSAnim {
    time: f32,
    duration: f32,
    command: OSAnimType,
}

impl OSAnim {
    pub fn import(buf: &[u8; OS_ANIM_SIZE], fps: f32) -> Option<Self> {
        let spf = 1.0 / fps;
        let command = LittleEndian::read_u32(&buf[0..4]);
        let time = LittleEndian::read_i32(&buf[4..8]);
        let duration = LittleEndian::read_i32(&buf[8..12]);

        if command == 0 {
            return None;
        }

        assert!(
            command & 0xFF == command && command.count_ones() == 1,
            "OSAnim invalid command {:08X}",
            command
        );

        assert!(
            time >= 0,
            "OSAnim invalid time {} for command {:02}",
            time,
            command
        );

        return Some(Self {
            time: time as f32 * spf,
            duration: duration as f32 * spf,
            command: OSAnimType::import(command, &buf[12..]),
        });
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.time)?;
        writer.write_f32::<LittleEndian>(self.duration)?;
        self.command.write(writer)?;
        return Ok(());
    }
}

impl fmt::Display for OSAnim {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Time: {:.02}, Duration: {:.02} | {}",
            self.time, self.duration, self.command
        )
    }
}

pub enum OSDrawType {
    Text {
        id: u8,
        length: i32,
        position: Vec2,
        color: U8Vec4,
    },
    Quad {
        vertices: [Vec2; 4],
        colors: [U8Vec4; 4],
    },
    Line {
        start: Vec2,
        end: Vec2,
        color: U8Vec4,
    },
    SpriteDef {
        id: u8,
        start: Vec2,
        end: Vec2,
        start2: Vec2,
        end2: Vec2,
        color: U8Vec4,
    },
    LinesDef {
        id: u8,
        position: Vec2,
        angle: f32,
        scale: f32,
        color: U8Vec4,
    },
}

impl OSDrawType {
    pub fn import(command: u32, buf: &[u8]) -> Self {
        match command {
            1 => {
                let id = LittleEndian::read_u32(&buf[8..12]);
                assert!(id <= u8::MAX as u32);
                Self::Text {
                    id: id as u8,
                    position: Vec2::new(
                        LittleEndian::read_f32(&buf[0..4]),
                        LittleEndian::read_f32(&buf[4..8]),
                    ),
                    length: LittleEndian::read_i32(&buf[12..16]),
                    color: U8Vec4::from_slice(&buf[52..56]).zyxw(),
                }
            }
            2 => {
                let mut vertex_components: [f32; 8] = [0.0; _];
                LittleEndian::read_f32_into(&buf[0..32], vertex_components.as_mut_slice());
                let color_buf = &buf[32..48];

                Self::Quad {
                    vertices: array::from_fn(|i| {
                        Vec2::from_slice(&vertex_components[i * 2..i * 2 + 2])
                    }),
                    colors: array::from_fn(|i| {
                        U8Vec4::from_slice(&color_buf[i * 4..i * 4 + 4]).zyxw()
                    }),
                }
            }
            3 => {
                let mut vertex_components: [f32; 4] = [0.0; _];
                LittleEndian::read_f32_into(&buf[0..16], vertex_components.as_mut_slice());

                Self::Line {
                    start: Vec2::from_slice(&vertex_components[0..2]),
                    end: Vec2::from_slice(&vertex_components[2..4]),
                    color: U8Vec4::from_slice(&buf[16..20]).zyxw(),
                }
            }
            4 => {
                let mut vertex_components: [f32; 8] = [0.0; _];
                LittleEndian::read_f32_into(&buf[0..32], vertex_components.as_mut_slice());

                let id = LittleEndian::read_u32(&buf[36..40]);
                assert!(id <= u8::MAX as u32);

                let start = Vec2::from_slice(&vertex_components[0..2]);
                let end = Vec2::from_slice(&vertex_components[2..4]);
                let start2 = Vec2::from_slice(&vertex_components[4..6]);
                let end2 = Vec2::from_slice(&vertex_components[6..8]);

                // if start2 != Vec2::ZERO || end2 != Vec2::ZERO {
                //     println!(
                //         "Sprite | Start: {:.02}, End: {:.02} - Start2: {:.02}, End2: {:.02}",
                //         start, end, start2, end2
                //     );
                // }

                Self::SpriteDef {
                    id: id as u8,
                    start,
                    end,
                    start2,
                    end2,
                    color: U8Vec4::from_slice(&buf[32..36]).zyxw(),
                }
            }
            5 => {
                let id = LittleEndian::read_u32(&buf[20..24]);
                assert!(id <= u8::MAX as u32);

                Self::LinesDef {
                    id: id as u8,
                    position: Vec2::new(
                        LittleEndian::read_f32(&buf[0..4]),
                        LittleEndian::read_f32(&buf[4..8]),
                    ),
                    angle: LittleEndian::read_f32(&buf[12..16]),
                    scale: LittleEndian::read_f32(&buf[16..20]),
                    color: U8Vec4::from_slice(&buf[8..12]).zyxw(),
                }
            }
            _ => todo!("unknown os draw type {:02X}", command),
        }
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            Self::Text {
                id,
                length,
                position,
                color,
            } => {
                writer.write_u8(0)?;
                writer.write_u8(*id)?;
                writer.write_i32::<LittleEndian>(*length)?;
                writer.write_f32::<LittleEndian>(position.x)?;
                writer.write_f32::<LittleEndian>(position.y)?;
                writer.write_all(color.to_array().as_slice())?;
            }
            Self::Quad { vertices, colors } => {
                writer.write_u8(1)?;
                for vertex in vertices.iter() {
                    writer.write_f32::<LittleEndian>(vertex.x)?;
                    writer.write_f32::<LittleEndian>(vertex.y)?;
                }
                for color in colors.iter() {
                    writer.write_all(color.to_array().as_slice())?;
                }
            }
            Self::Line { start, end, color } => {
                writer.write_u8(2)?;
                writer.write_f32::<LittleEndian>(start.x)?;
                writer.write_f32::<LittleEndian>(start.y)?;
                writer.write_f32::<LittleEndian>(end.x)?;
                writer.write_f32::<LittleEndian>(end.y)?;
                writer.write_all(color.to_array().as_slice())?;
            }
            Self::SpriteDef {
                id,
                start,
                end,
                start2,
                end2,
                color,
            } => {
                writer.write_u8(3)?;
                writer.write_u8(*id)?;
                writer.write_f32::<LittleEndian>(start.x)?;
                writer.write_f32::<LittleEndian>(start.y)?;
                writer.write_f32::<LittleEndian>(end.x)?;
                writer.write_f32::<LittleEndian>(end.y)?;
                writer.write_f32::<LittleEndian>(start2.x)?;
                writer.write_f32::<LittleEndian>(start2.y)?;
                writer.write_f32::<LittleEndian>(end2.x)?;
                writer.write_f32::<LittleEndian>(end2.y)?;
                writer.write_all(color.to_array().as_slice())?;
            }
            Self::LinesDef {
                id,
                position,
                angle,
                scale,
                color,
            } => {
                writer.write_u8(4)?;
                writer.write_u8(*id)?;
                writer.write_f32::<LittleEndian>(position.x)?;
                writer.write_f32::<LittleEndian>(position.y)?;
                writer.write_f32::<LittleEndian>(*angle)?;
                writer.write_f32::<LittleEndian>(*scale)?;
                writer.write_all(color.to_array().as_slice())?;
            }
        }
        return Ok(());
    }
}

impl fmt::Display for OSDrawType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Text {
                id,
                length,
                position,
                color,
            } => write!(
                f,
                "Text - ID {:02}, Length: {}, Position: {:.02}, Color: {}",
                id, length, position, color
            ),
            Self::Quad { vertices, colors } => write!(
                f,
                "Quad - V0 {:.02}, V1 {:.02}, V2 {:.02}, V3 {:.02} - C0 {}, C1 {}, C2 {}, C3 {}",
                vertices[0],
                vertices[1],
                vertices[2],
                vertices[3],
                colors[0],
                colors[1],
                colors[2],
                colors[3],
            ),
            Self::Line { start, end, color } => write!(
                f,
                "Line - Start {:.02}, End: {:.02}, Color: {}",
                start, end, color
            ),
            Self::SpriteDef {
                id,
                start,
                end,
                start2,
                end2,
                color,
            } => write!(
                f,
                "SpriteDef - ID: {:02}, Start: {:.02}, End: {:.02}, Start2: {:.02}, End2: {:.02}, Color: {}",
                id, start, end, start2, end2, color
            ),
            Self::LinesDef {
                id,
                position,
                angle,
                scale,
                color,
            } => write!(
                f,
                "LinesDef - ID: {:02}, Position: {:.02}, Angle: {:.02} Scale: {:.02}, Color {}",
                id, position, angle, scale, color
            ),
        }
    }
}

const OS_DRAW_SIZE: usize = 64;
const OS_DRAW_COUNT: usize = 112;
const OS_DRAW_ANIM_COUNT: usize = 16;
pub struct OSDraw {
    pub command: OSDrawType,
    pub animations: Vec<OSAnim>,
}

impl fmt::Display for OSDraw {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", self.command)?;
        for (i, animation) in self.animations.iter().enumerate() {
            writeln!(f, "  Animation {:02} | {}", i, animation)?;
        }
        return Ok(());
    }
}

impl OSDraw {
    pub fn import(buf: &[u8; OS_DRAW_SIZE]) -> Option<(Self, usize)> {
        let command = LittleEndian::read_u32(&buf[0..4]);
        let animation_count_raw = LittleEndian::read_i32(&buf[4..8]);
        if animation_count_raw <= 0 || command == 0 {
            assert!(
                command == 0,
                "OSDraw command non-zero {:08X}, with null animation count",
                command
            );
            return None;
        }

        let animation_count = cmp::min(animation_count_raw, 16) as usize;

        return Some((
            Self {
                command: OSDrawType::import(command, &buf[8..]),
                animations: Vec::with_capacity(animation_count),
            },
            animation_count,
        ));
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        self.command.write(writer)?;
        writer.write_u32::<LittleEndian>(self.animations.len() as u32)?;
        for anim in self.animations.iter() {
            anim.write(writer)?;
        }
        return Ok(());
    }
}

pub struct OS {
    pub duration: f32,
    pub draw_commands: Vec<OSDraw>,
}

impl OS {
    pub fn import(buf: &[u8], fps: f32) -> Self {
        let spf = 1.0 / fps;
        let duration = LittleEndian::read_u32(&buf[0..4]) as f32 * spf;

        let draw_buf_end = 4 + OS_DRAW_SIZE * OS_DRAW_COUNT;
        //let anim_buf_end = draw_buf_end + OS_ANIM_SIZE * OS_ANIM_COUNT;

        let (draw_buf_list, []) = buf[4..draw_buf_end].as_chunks::<OS_DRAW_SIZE>() else {
            panic!("buf[4..] length not a multiple of OS_DRAW_SIZE");
        };

        assert!(draw_buf_list.len() == OS_DRAW_COUNT);

        let (anim_buf_list, []) = buf[draw_buf_end..].as_chunks::<OS_ANIM_SIZE>() else {
            panic!("buf[draw_buf_end..] length not a multiple of OS_ANIM_SIZE");
        };

        assert!(anim_buf_list.len() == OS_ANIM_COUNT);

        let mut draw_commands: Vec<OSDraw> = Vec::with_capacity(OS_DRAW_COUNT);
        for (i, draw_buf) in draw_buf_list.iter().enumerate() {
            let anim_idx = i * OS_DRAW_ANIM_COUNT;
            let anim_bufs = &anim_buf_list[anim_idx..anim_idx + OS_DRAW_ANIM_COUNT];

            let Some((mut draw, anim_count)) = OSDraw::import(draw_buf) else {
                continue;
            };

            for anim_buf in anim_bufs[..anim_count].iter() {
                if let Some(anim) = OSAnim::import(anim_buf, fps) {
                    draw.animations.push(anim);
                };
            }

            draw_commands.push(draw);
        }

        assert!(anim_buf_list.len() == OS_ANIM_COUNT);

        return Self {
            duration,
            draw_commands,
        };
    }

    pub fn import_bin(path: &Path, fps: f32) -> Result<Vec<Self>, std::io::Error> {
        let file = File::open(path)?;
        let mut os_bin = BIN::from_reader(BufReader::new(file))?;

        println!("Importing {} OS files", os_bin.file_count());

        let mut os_list: Vec<Self> = Vec::with_capacity(os_bin.file_count());
        for file_index in 0..os_bin.file_count() {
            let bytes = os_bin.file_as_bytes(file_index)?;
            let os = Self::import(bytes.as_slice(), fps);

            os_list.push(os);
        }

        return Ok(os_list);
    }

    pub fn export(
        &self,
        path: &Path,
        font_path: &Path,
        strings_path: &Path,
        texture_path: &Path,
        spritesheet_path: &Path,
        lines_path: &Path,
    ) -> Result<(), std::io::Error> {
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        write_godot_path(font_path, &mut writer)?;
        write_godot_path(strings_path, &mut writer)?;
        write_godot_path(texture_path, &mut writer)?;
        write_godot_path(spritesheet_path, &mut writer)?;
        write_godot_path(lines_path, &mut writer)?;

        writer.write_f32::<LittleEndian>(self.duration)?;
        writer.write_u32::<LittleEndian>(self.draw_commands.len() as u32)?;
        for draw in self.draw_commands.iter() {
            draw.write(&mut writer)?;
        }

        return Ok(());
    }
}

impl fmt::Display for OS {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "Draw Command Count {}, Duration: {:.02}",
            self.draw_commands.len(),
            self.duration
        )?;
        for (i, draw) in self.draw_commands.iter().enumerate() {
            write!(f, "Draw Command {:03} | {}", i, draw)?;
        }
        return Ok(());
    }
}

pub struct Line(Vec2, Vec2);
pub type LinesDefs = Vec<Vec<Line>>;

impl Line {
    pub fn read(reader: &mut impl Read) -> Result<Self, std::io::Error> {
        let mut vertex_components: [f32; 4] = [0.0; _];
        reader.read_f32_into::<LittleEndian>(vertex_components.as_mut_slice())?;

        Ok(Line(
            Vec2::from_slice(&vertex_components[0..2]),
            Vec2::from_slice(&vertex_components[2..4]),
        ))
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.0.x)?;
        writer.write_f32::<LittleEndian>(self.0.y)?;
        writer.write_f32::<LittleEndian>(self.1.x)?;
        writer.write_f32::<LittleEndian>(self.1.y)?;
        return Ok(());
    }

    pub fn import_linesdefs(
        xbe: &mut XBE,
        section_name: &str,
        offset: u32,
        entry_count: usize,
    ) -> Result<LinesDefs, std::io::Error> {
        xbe.seek_section_offset(section_name, offset)?;

        let mut lines_entries_data: Vec<(usize, u32)> = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let count = xbe.reader.read_u32::<LittleEndian>()?;
            let ptr = xbe.reader.read_u32::<LittleEndian>()?;
            lines_entries_data.push((count as usize, ptr));
        }

        let mut lines_entries: Vec<Vec<Line>> = Vec::with_capacity(entry_count);
        for (count, ptr) in lines_entries_data {
            let mut lines: Vec<Line> = Vec::with_capacity(count);

            if count > 0 {
                xbe.seek_pointer_offset(ptr)?;
                for _ in 0..count {
                    let line = Line::read(&mut xbe.reader)?;
                    lines.push(line);
                }
            }

            lines_entries.push(lines);
        }

        return Ok(lines_entries);
    }

    pub fn export_linesdefs(lines_entries: &LinesDefs, path: &Path) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writer.write_u32::<LittleEndian>(lines_entries.len() as u32)?;
        for lines in lines_entries.iter() {
            writer.write_u32::<LittleEndian>(lines.len() as u32)?;
            for line in lines.iter() {
                line.write(&mut writer)?;
            }
        }

        return Ok(());
    }
}

enum OSSpriteColor {
    Pallete(u32),
    Color(U8Vec4),
}

pub const OS_SPRITE_SIZE: usize = 64;
pub const OS_SPRITE_SHORT_SIZE: usize = 24;
pub struct OSSprite {
    ui_idx: u32,
    frame_idx: u32,
    position: Vec2,
    origin: Vec2,
    size: Vec2,
    rotation: f32,
    scale: f32,

    color: OSSpriteColor,
}

impl OSSprite {
    pub fn import(buf: &[u8; OS_SPRITE_SIZE], color_pointers: &[u32]) -> Self {
        let ui_idx = LittleEndian::read_u32(&buf[0..4]);
        let frame_idx = LittleEndian::read_u32(&buf[4..8]);

        let mut vertex_components: [f32; 6] = [0.0; _];
        LittleEndian::read_f32_into(&buf[8..32], &mut vertex_components);

        let color_pointer = LittleEndian::read_u32(&buf[52..56]);

        let pallete_idx = color_pointers
            .iter()
            .position(|&ptr| ptr == color_pointer)
            .expect(format!("Unknown color pointer address: {:08X}", color_pointer).as_str());

        return Self {
            ui_idx,
            frame_idx,
            position: Vec2::from_slice(&vertex_components[0..2]),
            origin: Vec2::from_slice(&vertex_components[2..4]),
            size: Vec2::from_slice(&vertex_components[4..6]),
            rotation: LittleEndian::read_f32(&buf[32..36]),
            scale: LittleEndian::read_f32(&buf[36..40]),
            color: OSSpriteColor::Pallete(pallete_idx as u32),
        };
    }

    pub fn import_short(buf: &[u8; OS_SPRITE_SHORT_SIZE], ui_idx: u32) -> Self {
        let frame_idx = LittleEndian::read_u32(&buf[0..4]);

        let mut vertex_components: [f32; 6] = [0.0; _];
        LittleEndian::read_f32_into(&buf[4..20], &mut vertex_components);

        let size = Vec2::from_slice(&vertex_components[2..4]);

        let color = U8Vec4::from_slice(&buf[20..24]).zyxw();

        return Self {
            ui_idx,
            frame_idx,
            position: Vec2::from_slice(&vertex_components[0..2]),
            origin: size * 0.5,
            size,
            rotation: 0.0,
            scale: 1.0,
            color: OSSpriteColor::Color(color),
        };
    }

    pub fn import_multiple(
        xbe: &mut XBE,
        section_name: &str,
        offset: u32,
        count: usize,
        color_pointers: &[u32],
    ) -> Result<Vec<Self>, std::io::Error> {
        xbe.seek_section_offset(section_name, offset)?;

        let mut sprites: Vec<Self> = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buf: [u8; OS_SPRITE_SIZE] = [0; _];
            xbe.reader.read_exact(&mut buf)?;

            let sprite = Self::import(&buf, color_pointers);
            sprites.push(sprite);
        }

        return Ok(sprites);
    }

    pub fn import_multiple_short(
        xbe: &mut XBE,
        section_name: &str,
        offset: u32,
        count: usize,
        ui_idx: u32,
    ) -> Result<Vec<Self>, std::io::Error> {
        xbe.seek_section_offset(section_name, offset)?;

        let mut sprites: Vec<Self> = Vec::with_capacity(count);
        for _ in 0..count {
            let mut buf: [u8; OS_SPRITE_SHORT_SIZE] = [0; _];
            xbe.reader.read_exact(&mut buf)?;

            let sprite = Self::import_short(&buf, ui_idx);
            sprites.push(sprite);
        }

        return Ok(sprites);
    }

    pub fn write(
        &self,
        writer: &mut impl Write,
        ui_path_fn: FnIndexToPath,
    ) -> Result<(), std::io::Error> {
        let ui_path = ui_path_fn(self.ui_idx as usize);
        write_godot_path(&ui_path, writer)?;
        writer.write_u32::<LittleEndian>(self.frame_idx)?;

        writer.write_f32::<LittleEndian>(self.position.x)?;
        writer.write_f32::<LittleEndian>(self.position.y)?;
        writer.write_f32::<LittleEndian>(self.origin.x)?;
        writer.write_f32::<LittleEndian>(self.origin.y)?;
        writer.write_f32::<LittleEndian>(self.size.x)?;
        writer.write_f32::<LittleEndian>(self.size.y)?;
        writer.write_f32::<LittleEndian>(self.rotation)?;
        writer.write_f32::<LittleEndian>(self.scale)?;
        match self.color {
            OSSpriteColor::Pallete(pallete_idx) => {
                writer.write_u8(0)?;
                writer.write_u32::<LittleEndian>(pallete_idx)?;
            }
            OSSpriteColor::Color(color) => {
                writer.write_u8(1)?;
                writer.write_all(color.to_array().as_slice())?;
            }
        }

        return Ok(());
    }

    pub fn export_multiple(
        path: &Path,
        sprites: &[Self],
        ui_path_fn: FnIndexToPath,
    ) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writer.write_u32::<LittleEndian>(sprites.len() as u32)?;
        for sprite in sprites.iter() {
            sprite.write(&mut writer, ui_path_fn)?;
        }

        return Ok(());
    }
}
