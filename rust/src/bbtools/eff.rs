use std::cmp;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use byteorder::{ReadBytesExt, WriteBytesExt};
use glam::U8Vec4;
use glam::f32::Vec2;
use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD
use glam::f32::Vec4;

use byteorder::{ByteOrder, LittleEndian};

use crate::bbtools::bin::BIN;
use crate::bbtools::write_godot_path;
use crate::bbtools::xbe::XBE;
use crate::titles::FnIndexToPath;

struct EffectRepeat {
    repeat_count: u16,
    repeat_count_random: u16,
    repeat_interval: f32,
    vertex_color_random: [u8; 4],
    life_random: f32,
    initial_position_random: Vec3,
    initial_scale_xy_random: f32,
    initial_rotation_z_random: f32,
    velocity_position_rotation_random: Vec3,
    velocity_position_offset_random: Vec3,
}

impl EffectRepeat {
    pub fn import_rep(free: [f32; 36], scale: f32, fps: f32) -> Self {
        let spf = 1.0 / fps;

        let repeat_count = cmp::max(free[0] as i32, 1);
        assert!(
            repeat_count <= u16::MAX as i32,
            "Repeat Count {}",
            repeat_count
        );

        let initial_position_random = if free[2] != 0.0 {
            Vec3::new(0.0, 0.0, free[2])
        } else {
            Vec3::new(free[10], free[11], free[21])
        }
        .abs();

        let vertex_color_random = [
            free[15].abs(),
            free[16].abs(),
            free[17].abs(),
            free[18].abs(),
        ];
        for vc in vertex_color_random {
            assert!(vc as u64 <= u8::MAX as u64);
        }

        return EffectRepeat {
            repeat_count: repeat_count as u16,
            repeat_count_random: 0,
            repeat_interval: free[1] * spf,
            vertex_color_random: vertex_color_random.map(|vc| vc as u8),
            life_random: free[4] * spf,
            initial_position_random: initial_position_random * scale,
            initial_scale_xy_random: free[3].abs() * scale,
            initial_rotation_z_random: free[9].to_radians().abs(),
            velocity_position_rotation_random: Vec3::new(
                free[12].to_radians(),
                free[13].to_radians(),
                free[14].to_radians(),
            )
            .abs(),
            velocity_position_offset_random: Vec3::new(free[5], free[19], free[20]) * scale * fps,
        };
    }

    pub fn import_par(free: [f32; 36], scale: f32, fps: f32) -> Self {
        let spf = 1.0 / fps;

        let repeat_count = cmp::max(free[0] as i32, 1);
        assert!(
            repeat_count <= u16::MAX as i32,
            "Repeat Count {}",
            repeat_count
        );

        let repeat_count_random = free[1] as i32;
        assert!(repeat_count_random >= 0 && repeat_count_random <= u16::MAX as i32);

        let vertex_color_random = [
            free[12].abs(),
            free[13].abs(),
            free[14].abs(),
            free[15].abs(),
        ];
        for vc in vertex_color_random {
            assert!(vc as u64 <= u8::MAX as u64);
        }

        return EffectRepeat {
            repeat_count: repeat_count as u16,
            repeat_count_random: repeat_count_random as u16,
            repeat_interval: free[1] * spf,
            vertex_color_random: vertex_color_random.map(|vc| vc as u8),
            life_random: free[11] * spf,
            initial_position_random: Vec3::new(free[2], 0.0, free[2]).abs() * scale,
            initial_scale_xy_random: free[10].abs() * scale,
            initial_rotation_z_random: free[9].to_radians().abs(),
            velocity_position_rotation_random: Vec3::new(
                free[3].to_radians(),
                free[4].to_radians(),
                free[5].to_radians(),
            )
            .abs(),
            velocity_position_offset_random: Vec3::new(free[16], free[17], free[18]) * scale * fps,
        };
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u16::<LittleEndian>(self.repeat_count)?;
        writer.write_u16::<LittleEndian>(self.repeat_count_random)?;
        writer.write_f32::<LittleEndian>(self.repeat_interval)?;
        for vcr in self.vertex_color_random {
            writer.write_u8(vcr)?;
        }

        writer.write_f32::<LittleEndian>(self.life_random)?;

        writer.write_f32::<LittleEndian>(self.initial_position_random.x)?;
        writer.write_f32::<LittleEndian>(self.initial_position_random.y)?;
        writer.write_f32::<LittleEndian>(self.initial_position_random.z)?;

        writer.write_f32::<LittleEndian>(self.initial_scale_xy_random)?;
        writer.write_f32::<LittleEndian>(self.initial_rotation_z_random)?;

        writer.write_f32::<LittleEndian>(self.velocity_position_rotation_random.x)?;
        writer.write_f32::<LittleEndian>(self.velocity_position_rotation_random.y)?;
        writer.write_f32::<LittleEndian>(self.velocity_position_rotation_random.z)?;

        writer.write_f32::<LittleEndian>(self.velocity_position_offset_random.x)?;
        writer.write_f32::<LittleEndian>(self.velocity_position_offset_random.y)?;
        writer.write_f32::<LittleEndian>(self.velocity_position_offset_random.z)?;
        return Ok(());
    }
}

#[allow(unused, non_camel_case_types)]
enum EffectType {
    SIN_2D(u8, u8), // UV, SEQ
    REP_2D(u8, u8, EffectRepeat),
    PAR_2D(u8, u8, EffectRepeat),
    PAR_LIN(u8, u8),
    GK,
    SIN_3D(u16),
    REP_3D(u16, EffectRepeat),
    PAR_3D(u16, EffectRepeat),
}

impl EffectType {
    pub fn import(
        val: u16,
        uv: u8,
        seq: u8,
        model: u16,
        free: [f32; 36],
        scale: f32,
        fps: f32,
    ) -> Self {
        match val {
            0 => EffectType::SIN_2D(uv, seq),
            1 => EffectType::REP_2D(uv, seq, EffectRepeat::import_rep(free, scale, fps)),
            2 => EffectType::PAR_2D(uv, seq, EffectRepeat::import_par(free, scale, fps)),
            3 => EffectType::PAR_LIN(uv, seq),
            //4 => EffectType::GK, // Unused
            5 => EffectType::SIN_3D(model),
            6 => EffectType::REP_3D(model, EffectRepeat::import_rep(free, scale, fps)),
            7 => EffectType::PAR_3D(model, EffectRepeat::import_par(free, scale, fps)),
            _ => todo!("Unknown EffectType"),
        }
    }

    pub fn as_u8(&self) -> u8 {
        match self {
            EffectType::SIN_2D(_, _) => 0,
            EffectType::REP_2D(_, _, _) => 1,
            EffectType::PAR_2D(_, _, _) => 2,
            EffectType::PAR_LIN(_, _) => 3,
            EffectType::GK => 4,
            EffectType::SIN_3D(_) => 5,
            EffectType::REP_3D(_, _) => 6,
            EffectType::PAR_3D(_, _) => 7,
        }
    }

    pub fn write_type(
        &self,
        uv_path_fn: FnIndexToPath,
        seq_path_fn: FnIndexToPath,
        model_path_fn: FnIndexToPath,
        writer: &mut impl Write,
    ) -> Result<(), std::io::Error> {
        writer.write_u8(self.as_u8())?;
        match self {
            EffectType::SIN_2D(uv, seq)
            | EffectType::REP_2D(uv, seq, _)
            | EffectType::PAR_2D(uv, seq, _)
            | EffectType::PAR_LIN(uv, seq) => {
                let uv_path = uv_path_fn(*uv as usize);
                write_godot_path(&uv_path, writer)?;

                let seq_path = seq_path_fn(*seq as usize);
                write_godot_path(&seq_path, writer)?;
            }
            EffectType::SIN_3D(model)
            | EffectType::REP_3D(model, _)
            | EffectType::PAR_3D(model, _) => {
                let model_path = model_path_fn(*model as usize);
                write_godot_path(&model_path, writer)?;
            }
            _ => todo!("Unknown EffectType"),
        }

        return Ok(());
    }

    pub fn write_repeat(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        match self {
            EffectType::REP_2D(_, _, repeat)
            | EffectType::PAR_2D(_, _, repeat)
            | EffectType::REP_3D(_, repeat)
            | EffectType::PAR_3D(_, repeat) => {
                repeat.write(writer)?;
            }
            _ => {}
        }
        return Ok(());
    }
}

#[derive(Clone, Copy)]
enum BlendType {
    ALPHA = 0,
    ADD = 1,
    SUB = 2, // Unused
}

impl From<u8> for BlendType {
    fn from(val: u8) -> Self {
        match val {
            0 => BlendType::ALPHA,
            1 => BlendType::ADD,
            2 => BlendType::SUB,
            _ => todo!("Unknown BlendType"),
        }
    }
}

const TRANSFORM_SIZE: usize = 36;
pub struct Transform {
    position: Vec3,
    rotation: Vec3,
    scale: Vec3,
}

impl Transform {
    pub fn new(buf: &[u8], scale: f32, fps: f32) -> Self {
        assert!(buf.len() == TRANSFORM_SIZE);

        return Transform {
            position: Vec3::new(
                LittleEndian::read_f32(&buf[0..4]),
                LittleEndian::read_f32(&buf[4..8]),
                LittleEndian::read_f32(&buf[8..12]),
            ) * scale
                * fps,
            rotation: Vec3::new(
                LittleEndian::read_f32(&buf[12..16]),
                LittleEndian::read_f32(&buf[16..20]),
                LittleEndian::read_f32(&buf[20..24]),
            ) * fps,
            scale: Vec3::new(
                LittleEndian::read_f32(&buf[24..28]),
                LittleEndian::read_f32(&buf[28..32]),
                LittleEndian::read_f32(&buf[32..36]),
            ) * scale
                * fps,
        };
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.position.x)?;
        writer.write_f32::<LittleEndian>(self.position.y)?;
        writer.write_f32::<LittleEndian>(self.position.z)?;

        writer.write_f32::<LittleEndian>(self.rotation.x)?;
        writer.write_f32::<LittleEndian>(self.rotation.y)?;
        writer.write_f32::<LittleEndian>(self.rotation.z)?;

        writer.write_f32::<LittleEndian>(self.scale.x)?;
        writer.write_f32::<LittleEndian>(self.scale.y)?;
        writer.write_f32::<LittleEndian>(self.scale.z)?;
        return Ok(());
    }
}

const EFFECT_SIZE: usize = 308;
pub struct Effect {
    effect_type: EffectType,
    blend_type: BlendType,
    life: f32,
    delay: f32,
    priority: u16,
    flags: u16,
    child_effect: Option<(u16, f32)>, // id, interval
    vertex_color: [u8; 4],            // RGBA
    damping_color: [f32; 4],          // RGBA
    damping_duration: f32,
    initial_transform: Transform,
    velocity_transform: Transform,
    acceleration_transform: Transform,
    acceleration_gravity: Vec3,
}

impl Effect {
    pub fn import(buf: &[u8], model_list: &[u16], scale: f32, fps: f32) -> Self {
        assert!(buf.len() == EFFECT_SIZE);
        let spf = 1.0 / fps;

        let blend_type = buf[2];
        assert!(blend_type < 3);

        assert!(buf[3] == 0); // JNT (not used)

        assert!(LittleEndian::read_u16(&buf[6..8]) == 0); // Texture (not used)

        let uv = LittleEndian::read_u16(&buf[8..10]);
        assert!(uv <= u8::MAX as u16);

        let seq = LittleEndian::read_u16(&buf[10..12]);
        assert!(seq <= u8::MAX as u16);

        let model_idx = LittleEndian::read_u16(&buf[12..14]) as usize;

        let life = LittleEndian::read_u32(&buf[36..40]);
        assert!(life < u16::MAX as u32);

        let mut free: [f32; 36] = [0.0; _];
        LittleEndian::read_f32_into(&buf[164..308], &mut free);

        let child_effect_id = free[6] as i32;
        assert!(child_effect_id >= 0 && child_effect_id <= u16::MAX as i32);

        let child_effect = if child_effect_id > 0 {
            let child_effect_interval = free[7] * spf;
            Some((child_effect_id as u16, child_effect_interval))
        } else {
            None
        };

        let damping_duration = free[8] * spf;

        return Effect {
            effect_type: EffectType::import(
                LittleEndian::read_u16(&buf[0..2]),
                uv as u8,
                seq as u8,
                model_list[model_idx],
                free,
                scale,
                fps,
            ),
            blend_type: BlendType::from(blend_type),
            life: if life == 0 {
                f32::INFINITY
            } else {
                life as f32 * spf
            },
            delay: LittleEndian::read_u32(&buf[40..44]) as f32 * spf,
            priority: LittleEndian::read_u16(&buf[4..6]),
            flags: LittleEndian::read_u16(&buf[14..16]),

            child_effect,

            vertex_color: [
                buf[18], // Vertex Color R
                buf[17], // Vertex Color G
                buf[16], // Vertex Color B
                buf[19], // Vertex Color A
            ],

            damping_color: [
                LittleEndian::read_f32(&buf[20..24]), // Damping Color B
                LittleEndian::read_f32(&buf[24..28]), // Damping Color G
                LittleEndian::read_f32(&buf[28..32]), // Damping Color R
                LittleEndian::read_f32(&buf[32..36]), // Damping Color A
            ],
            damping_duration,

            initial_transform: Transform::new(&buf[44..80], scale, 1.0),
            velocity_transform: Transform::new(&buf[80..116], scale, fps),
            acceleration_transform: Transform::new(&buf[116..152], scale, fps * fps),

            acceleration_gravity: Vec3::new(
                LittleEndian::read_f32(&buf[152..156]),
                LittleEndian::read_f32(&buf[156..160]),
                LittleEndian::read_f32(&buf[160..164]),
            ) * (scale * fps * fps),
        };
    }

    pub fn write(
        &self,
        efe_path_fn: FnIndexToPath,
        uv_path_fn: FnIndexToPath,
        efs_path_fn: FnIndexToPath,
        model_path_fn: FnIndexToPath,
        writer: &mut impl Write,
    ) -> Result<(), std::io::Error> {
        self.effect_type
            .write_type(uv_path_fn, efs_path_fn, model_path_fn, writer)?;
        writer.write_u8(self.blend_type as u8)?;
        writer.write_f32::<LittleEndian>(self.life)?;
        writer.write_f32::<LittleEndian>(self.delay)?;
        writer.write_u16::<LittleEndian>(self.priority)?;
        writer.write_u16::<LittleEndian>(self.flags)?;

        if let Some((c_id, c_interval)) = self.child_effect {
            let c_efe_path = efe_path_fn(c_id as usize);
            write_godot_path(&c_efe_path, writer)?;
            writer.write_f32::<LittleEndian>(c_interval)?;
        } else {
            writer.write_u32::<LittleEndian>(0)?; // Empty Pascal String
        }

        for vc in self.vertex_color {
            writer.write_u8(vc)?;
        }

        for dc in self.damping_color {
            writer.write_f32::<LittleEndian>(dc)?;
        }
        writer.write_f32::<LittleEndian>(self.damping_duration)?;

        self.initial_transform.write(writer)?;
        self.velocity_transform.write(writer)?;
        self.acceleration_transform.write(writer)?;

        writer.write_f32::<LittleEndian>(self.acceleration_gravity.x)?;
        writer.write_f32::<LittleEndian>(self.acceleration_gravity.y)?;
        writer.write_f32::<LittleEndian>(self.acceleration_gravity.z)?;

        // This block is only written if this is a repeating type
        self.effect_type.write_repeat(writer)?;
        return Ok(());
    }
}

pub struct EFE {
    pub id: u16,
    effects: Vec<Effect>,
}

impl EFE {
    pub fn import(buf: &[u8], id: u16, model_list: &[u16], scale: f32, fps: f32) -> Self {
        let effect_count = LittleEndian::read_u32(&buf[0..4]) as usize;

        let effect_offsets_end = 4 + effect_count * 4;
        let mut effect_offsets: Vec<u32> = vec![0; effect_count];
        LittleEndian::read_u32_into(&buf[4..effect_offsets_end], &mut effect_offsets);

        let mut effects: Vec<Effect> = Vec::with_capacity(effect_count);
        for eo in effect_offsets {
            let effect_offset = eo as usize;
            let effect_buf = &buf[effect_offset..effect_offset + EFFECT_SIZE];

            effects.push(Effect::import(effect_buf, model_list, scale, fps));
        }

        return EFE {
            id: id,
            effects: effects,
        };
    }

    pub fn export(
        &self,
        path: &Path,
        efe_path_fn: FnIndexToPath,
        uv_path_fn: FnIndexToPath,
        efs_path_fn: FnIndexToPath,
        model_path_fn: FnIndexToPath,
    ) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writer.write_u32::<LittleEndian>(self.effects.len() as u32)?;
        for effect in self.effects.iter() {
            effect.write(
                efe_path_fn,
                uv_path_fn,
                efs_path_fn,
                model_path_fn,
                &mut writer,
            )?;
        }

        return Ok(());
    }
}

enum SequenceFrameType {
    Delay(f32), // How long in seconds to display the frame for
    Exit,
    Reset,
    Pause,
}

impl SequenceFrameType {
    pub fn import(val: i16, spf: f32) -> Self {
        match val {
            -1 => SequenceFrameType::Exit,
            -2 => SequenceFrameType::Reset,
            -3 => SequenceFrameType::Pause,
            _ => {
                assert!(val > 0);
                SequenceFrameType::Delay(val as f32 * spf)
            }
        }
    }
}

const SEQUENCE_FRAME_SIZE: usize = 4;
pub struct SequenceFrame {
    frame_idx: u16, // Index into a UV struct's frames
    frame_type: SequenceFrameType,
}

impl SequenceFrame {
    pub fn import(buf: &[u8], spf: f32) -> Self {
        let value = LittleEndian::read_i16(&buf[2..4]);
        return SequenceFrame {
            frame_idx: LittleEndian::read_u16(&buf[0..2]),
            frame_type: SequenceFrameType::import(value, spf),
        };
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u16::<LittleEndian>(self.frame_idx)?;
        match self.frame_type {
            SequenceFrameType::Delay(delay) => {
                writer.write_u8(0)?;
                writer.write_f32::<LittleEndian>(delay)?;
            }
            SequenceFrameType::Exit => {
                writer.write_u8(1)?;
            }
            SequenceFrameType::Reset => {
                writer.write_u8(2)?;
            }
            SequenceFrameType::Pause => {
                writer.write_u8(3)?;
            }
        }
        return Ok(());
    }
}

pub struct SEQ {
    pub idx: usize,
    frames: Vec<SequenceFrame>,
}

impl SEQ {
    pub fn import(buf: &[u8], idx: usize, fps: f32) -> Self {
        let spf = 1.0 / fps;

        let frame_count = LittleEndian::read_u32(&buf[0..4]) as usize;

        let mut frame_buf = &buf[4..];

        let mut frames: Vec<SequenceFrame> = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let frame = SequenceFrame::import(&frame_buf[..SEQUENCE_FRAME_SIZE], spf);
            let is_delay_frame = matches!(frame.frame_type, SequenceFrameType::Delay(_));
            frames.push(frame);

            if !is_delay_frame {
                break;
            }

            frame_buf = &frame_buf[SEQUENCE_FRAME_SIZE..];
        }

        return SEQ {
            idx: idx,
            frames: frames,
        };
    }

    pub fn export(&self, path: &Path) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writer.write_u16::<LittleEndian>(self.frames.len() as u16)?;
        for frame in self.frames.iter() {
            frame.write(&mut writer)?;
        }

        return Ok(());
    }
}

const UVFRAME_SIZE: usize = 24;
pub struct UVFrame {
    pub start: Vec2,
    pub end: Vec2,
    pub scale: Vec2,
}

impl UVFrame {
    pub fn import(buf: &[u8], scale: f32) -> Self {
        return UVFrame {
            start: Vec2::new(
                LittleEndian::read_f32(&buf[0..4]),
                LittleEndian::read_f32(&buf[4..8]),
            ),
            end: Vec2::new(
                LittleEndian::read_f32(&buf[8..12]),
                LittleEndian::read_f32(&buf[12..16]),
            ),
            scale: Vec2::new(
                LittleEndian::read_f32(&buf[16..20]),
                LittleEndian::read_f32(&buf[20..24]),
            ) * scale,
        };
    }

    pub fn import_cockpit(buf: &[u8], texture_size: Vec2) -> Self {
        return UVFrame {
            start: Vec2::new(
                LittleEndian::read_f32(&buf[0..4]),
                LittleEndian::read_f32(&buf[4..8]),
            ) / texture_size,
            end: Vec2::new(
                LittleEndian::read_f32(&buf[8..12]),
                LittleEndian::read_f32(&buf[12..16]),
            ) / texture_size,
            scale: Vec2::ONE,
        };
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.start.x)?;
        writer.write_f32::<LittleEndian>(self.start.y)?;
        writer.write_f32::<LittleEndian>(self.end.x)?;
        writer.write_f32::<LittleEndian>(self.end.y)?;
        writer.write_f32::<LittleEndian>(self.scale.x)?;
        writer.write_f32::<LittleEndian>(self.scale.y)?;
        return Ok(());
    }
}

pub struct UV {
    pub idx: usize,
    pub texture: u16,
    pub offset: Vec2,
    pub frames: Vec<UVFrame>,
}

impl UV {
    pub fn import(buf: &[u8], idx: usize, texture_list: &[u16], scale: f32) -> Self {
        let frame_count = LittleEndian::read_u32(&buf[0..4]) as usize;

        let texture_idx = LittleEndian::read_u32(&buf[4..8]) as usize;

        let mut frame_buf = &buf[16..];

        let mut frames: Vec<UVFrame> = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            frames.push(UVFrame::import(&frame_buf[..UVFRAME_SIZE], scale));
            frame_buf = &frame_buf[UVFRAME_SIZE..];
        }

        return UV {
            idx: idx,
            texture: texture_list[texture_idx],
            offset: Vec2::new(
                LittleEndian::read_f32(&buf[8..12]),
                LittleEndian::read_f32(&buf[12..16]),
            ),
            frames: frames,
        };
    }

    pub fn import_cockpit(
        path: &Path,
        texture_idx: u16,
        texture_size: Vec2,
    ) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let frame_count = reader.read_u32::<LittleEndian>()? as usize;

        assert!(reader.read_u32::<LittleEndian>()? == 0);

        let mut frame_buf: [u8; 16] = [0; _];

        let mut frames: Vec<UVFrame> = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            reader.read_exact(&mut frame_buf)?;
            frames.push(UVFrame::import_cockpit(frame_buf.as_slice(), texture_size));
        }

        return Ok(UV {
            idx: 0,
            texture: texture_idx,
            offset: Vec2::ZERO,
            frames,
        });
    }

    pub fn export(
        &self,
        path: &Path,
        texture_path_fn: FnIndexToPath,
    ) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let texture_path = texture_path_fn(self.texture as usize);
        write_godot_path(&texture_path, &mut writer)?;

        writer.write_f32::<LittleEndian>(self.offset.x)?;
        writer.write_f32::<LittleEndian>(self.offset.y)?;

        assert!(self.frames.len() <= u16::MAX as usize);
        writer.write_u16::<LittleEndian>(self.frames.len() as u16)?;
        for frame in self.frames.iter() {
            frame.write(&mut writer)?;
        }

        return Ok(());
    }
}

pub struct EFP {
    pub efe_list: Vec<EFE>,
    pub seq_list: Vec<SEQ>,
    pub uv1_list: Vec<UV>,
    pub uv2_list: Vec<UV>,
}

impl EFP {
    pub fn import(
        efp_path: &Path,
        model_list: &[u16],
        texture_list: &[u16],
        effect_ids: &[u32],
        scale: f32,
        fps: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        /*** Load Effect Pack ***/
        let efp_file = File::open(efp_path)?;
        let mut bin_efp = BIN::from_reader(BufReader::new(efp_file))?;
        assert!(bin_efp.file_count() == 4);

        let efe_cursor = Cursor::new(bin_efp.file_as_bytes(0)?);
        let seq_cursor = Cursor::new(bin_efp.file_as_bytes(1)?);
        let uv1_cursor = Cursor::new(bin_efp.file_as_bytes(2)?);
        let uv2_cursor = Cursor::new(bin_efp.file_as_bytes(3)?);

        let mut efe_bin = BIN::from_reader(BufReader::new(efe_cursor))?;
        let mut seq_bin = BIN::from_reader(BufReader::new(seq_cursor))?;
        let mut uv1_bin = BIN::from_reader(BufReader::new(uv1_cursor))?;
        let mut uv2_bin = BIN::from_reader(BufReader::new(uv2_cursor))?;

        /*** Process UV files ***/
        let mut uv1_list: Vec<UV> = Vec::with_capacity(uv1_bin.file_count());
        for file_index in 0..uv1_bin.file_count() {
            let file_bytes = uv1_bin.file_as_bytes(file_index)?;
            if file_bytes.len() == 0 {
                continue;
            }

            let uv = UV::import(file_bytes.as_slice(), file_index, texture_list, scale);
            uv1_list.push(uv);
        }
        println!(
            "Imported {} of {} UV files form UV1",
            uv1_list.len(),
            uv1_bin.file_count()
        );

        let mut uv2_list: Vec<UV> = Vec::with_capacity(uv2_bin.file_count());
        for file_index in 0..uv2_bin.file_count() {
            let file_bytes = uv2_bin.file_as_bytes(file_index)?;
            if file_bytes.len() == 0 {
                continue;
            }

            let uv = UV::import(file_bytes.as_slice(), file_index, texture_list, scale);
            uv2_list.push(uv);
        }
        println!(
            "Imported {} of {} UV files form UV2",
            uv2_list.len(),
            uv2_bin.file_count()
        );

        /*** Process SEQ files ***/
        let mut seq_list: Vec<SEQ> = Vec::with_capacity(seq_bin.file_count());
        for file_index in 0..seq_bin.file_count() {
            let file_bytes = seq_bin.file_as_bytes(file_index)?;
            if file_bytes.len() == 0 {
                continue;
            }

            let seq = SEQ::import(file_bytes.as_slice(), file_index, fps);
            seq_list.push(seq);
        }

        println!(
            "Imported {} of {} Sequence files form SEQ",
            seq_list.len(),
            seq_bin.file_count()
        );

        /*** Process EFE files ***/
        let mut efe_list: Vec<EFE> = Vec::with_capacity(efe_bin.file_count());
        for file_index in 0..efe_bin.file_count() {
            let id = effect_ids[file_index];
            assert!(id <= u16::MAX as u32);

            let file_bytes = efe_bin.file_as_bytes(file_index)?;
            if file_bytes.len() == 0 {
                continue;
            }

            let efe = EFE::import(file_bytes.as_slice(), id as u16, model_list, scale, fps);
            efe_list.push(efe);
        }

        println!(
            "Imported {} of {} Effect files form EFE",
            efe_list.len(),
            efe_bin.file_count()
        );

        return Ok(EFP {
            efe_list: efe_list,
            seq_list: seq_list,
            uv1_list: uv1_list,
            uv2_list: uv2_list,
        });
    }
}

#[derive(Clone, Copy)]
pub struct EffectPosition {
    pub idx: u16,
    pub position: Vec3,
}

impl EffectPosition {
    pub fn import(
        xbe: &mut XBE,
        section_name: &str,
        section_offset: u32,
        count: usize,
        scale: f32,
    ) -> Result<Vec<Option<Self>>, std::io::Error> {
        xbe.seek_section_offset(section_name, section_offset)?;

        let mut effpos_list: Vec<Option<Self>> = Vec::with_capacity(count);
        for _ in 0..count {
            let effect_idx = xbe.reader.read_u32::<LittleEndian>()?;
            effpos_list.push(if effect_idx == u32::MAX {
                None
            } else {
                assert!(effect_idx <= u16::MAX as u32);
                Some(Self {
                    idx: effect_idx as u16,
                    position: Vec3::new(
                        xbe.reader.read_f32::<LittleEndian>()?,
                        xbe.reader.read_f32::<LittleEndian>()?,
                        xbe.reader.read_f32::<LittleEndian>()?,
                    ) * scale,
                })
            });
        }

        return Ok(effpos_list);
    }

    pub fn import_separate(
        xbe: &mut XBE,
        idx_section_name: &str,
        idx_offset: u32,
        pos_section_name: &str,
        pos_offset: u32,
        count: usize,
        scale: f32,
    ) -> Result<Vec<Option<Self>>, std::io::Error> {
        xbe.seek_section_offset(idx_section_name, idx_offset)?;

        let mut effect_idx_list: Vec<u32> = vec![0; count];
        xbe.reader
            .read_u32_into::<LittleEndian>(&mut effect_idx_list)?;

        xbe.seek_section_offset(pos_section_name, pos_offset)?;

        let mut effpos_list: Vec<Option<Self>> = Vec::with_capacity(count);
        for effect_idx in effect_idx_list {
            let effect_pos = Vec3::new(
                xbe.reader.read_f32::<LittleEndian>()?,
                xbe.reader.read_f32::<LittleEndian>()?,
                xbe.reader.read_f32::<LittleEndian>()?,
            );

            effpos_list.push(if effect_idx == u32::MAX {
                None
            } else {
                assert!(effect_idx <= u16::MAX as u32);
                Some(Self {
                    idx: effect_idx as u16,
                    position: effect_pos * scale,
                })
            });
        }

        return Ok(effpos_list);
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u16::<LittleEndian>(self.idx)?;
        writer.write_f32::<LittleEndian>(self.position.x)?;
        writer.write_f32::<LittleEndian>(self.position.y)?;
        writer.write_f32::<LittleEndian>(self.position.z)?;

        return Ok(());
    }
}

impl fmt::Display for EffectPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Idx: {}, Position: {}", self.idx, self.position,)
    }
}

const EFFECT_LIGHT_SIZE: usize = 40;
pub struct EffectLight {
    life: f32,
    color_duration: f32,
    color_start: Vec4,
    color_end: Vec4,
}

impl EffectLight {
    pub fn import(buf: &[u8], scale: f32, fps: f32) -> Option<Self> {
        assert!(buf.len() == EFFECT_LIGHT_SIZE);
        let spf = 1.0 / fps;

        let flags = LittleEndian::read_u32(&buf[0..4]);
        if flags != 1 {
            return None;
        }

        let color_add_count = LittleEndian::read_u16(&buf[4..6]);
        let color_add = if color_add_count == 0 {
            Vec4::ZERO
        } else {
            Vec4::new(
                LittleEndian::read_f32(&buf[8..12]),
                LittleEndian::read_f32(&buf[12..16]),
                LittleEndian::read_f32(&buf[16..20]),
                LittleEndian::read_f32(&buf[20..24]),
            ) * scale
        };

        let color_start = Vec4::new(
            LittleEndian::read_f32(&buf[24..28]),
            LittleEndian::read_f32(&buf[28..32]),
            LittleEndian::read_f32(&buf[32..36]),
            LittleEndian::read_f32(&buf[36..40]),
        ) * scale;

        let color_end: Vec4 = color_start - color_add * color_add_count as f32;

        return Some(EffectLight {
            life: LittleEndian::read_u16(&buf[6..8]) as f32 * spf,
            color_duration: color_add_count as f32 * spf,
            color_start,
            color_end,
        });
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.life)?;
        writer.write_f32::<LittleEndian>(self.color_duration)?;
        for c in self.color_start.to_array() {
            writer.write_f32::<LittleEndian>(c)?;
        }
        for c in self.color_end.to_array() {
            writer.write_f32::<LittleEndian>(c)?;
        }

        return Ok(());
    }
}

pub struct LID {
    pub idx: usize,
    light: EffectLight,
}

impl LID {
    pub fn import(path: &Path, scale: f32, fps: f32) -> Result<Vec<Self>, std::io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let light_count = reader.read_u32::<LittleEndian>()? as usize;
        let light_size = reader.read_u32::<LittleEndian>()? as usize;
        if light_size != EFFECT_LIGHT_SIZE {
            return Err(std::io::Error::other("invalid light size"));
        }

        let mut light_buf: Vec<u8> = vec![0; light_size];

        let mut lid_list: Vec<LID> = Vec::with_capacity(light_count);
        for idx in 0..light_count {
            reader.read_exact(&mut light_buf)?;
            if let Some(light) = EffectLight::import(light_buf.as_slice(), scale, fps) {
                lid_list.push(LID { idx, light });
            }
        }

        println!(
            "Imported {} of {} effect lights from LID",
            lid_list.len(),
            light_count
        );

        return Ok(lid_list);
    }

    pub fn export(&self, path: &Path) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        self.light.write(&mut writer)?;

        return Ok(());
    }
}

impl fmt::Display for LID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Idx: {}, Life: {}, Duration: {}, Color Start: {}, Color End: {}",
            self.idx,
            self.light.life,
            self.light.color_duration,
            self.light.color_start,
            self.light.color_end
        )
    }
}

pub const SMOKE_TRAIL_SIZE: usize = 56;
pub const TRACER_TRAIL_SIZE: usize = 44;

pub struct TrailEffect {
    uv_idx: usize,
    color_start: U8Vec4, // RGBA
    color_max: U8Vec4,   // RGBA
    section_count: u8,
    texture_scale: f32,
    pub width_start: f32,
    pub width_end: f32,
    flags: u32,
}

impl TrailEffect {
    pub fn import_smoke(buf: &[u8; SMOKE_TRAIL_SIZE], uv_idx: usize, scale: f32) -> Self {
        let section_count = LittleEndian::read_i32(&buf[8..12]);
        assert!(section_count >= 0 && section_count <= u8::MAX as i32);

        let zero: [u8; 32] = [0; _];
        assert!(buf[24..56] == zero);

        return Self {
            uv_idx,
            color_start: U8Vec4::new(
                buf[1], // R
                buf[2], // G
                buf[3], // B
                buf[0], // A
            ),
            color_max: U8Vec4::new(
                buf[5], // R
                buf[6], // G
                buf[7], // B
                buf[4], // A
            ),
            section_count: section_count as u8,
            texture_scale: section_count as f32 * 0.25,
            width_start: LittleEndian::read_f32(&buf[12..16]) as f32 * scale,
            width_end: LittleEndian::read_f32(&buf[16..20]) as f32 * scale,
            flags: LittleEndian::read_u32(&buf[20..24]),
        };
    }

    pub fn import_tracer(buf: &[u8; TRACER_TRAIL_SIZE], uv_idx: usize, scale: f32) -> Self {
        let section_count = LittleEndian::read_i32(&buf[4..8]);
        assert!(section_count >= 0 && section_count <= u8::MAX as i32);

        let zero: [u8; 32] = [0; _];
        assert!(buf[12..44] == zero);

        assert!(buf[0] == 0); // Ignore the alpha value since it's always zero

        let color = U8Vec4::new(
            buf[1], // R
            buf[2], // G
            buf[3], // B
            0xFF,   // A
        );

        let size = LittleEndian::read_f32(&buf[8..12]) as f32 * scale;

        return Self {
            uv_idx,
            color_start: color,
            color_max: color / 3,
            section_count: section_count as u8,
            texture_scale: 1.0,
            width_start: size,
            width_end: size,
            flags: 0,
        };
    }

    pub fn new_tracer(uv_idx: usize, color: U8Vec4, section_count: u8, size: f32) -> Self {
        Self {
            uv_idx,
            color_start: color,
            color_max: color / 3,
            section_count: section_count as u8,
            texture_scale: 1.0,
            width_start: size,
            width_end: size,
            flags: 0,
        }
    }

    pub fn import_smoke_multiple(buf: &[u8], uv_idx: usize, scale: f32) -> Vec<Self> {
        let (smoke_list, []) = buf.as_chunks::<SMOKE_TRAIL_SIZE>() else {
            panic!("buf length not a multiple of SMOKE_TRAIL_SIZE");
        };

        let mut smoke_trail_list: Vec<Self> = Vec::with_capacity(smoke_list.len());
        for smoke_buf in smoke_list {
            let smoke = Self::import_smoke(smoke_buf, uv_idx, scale);
            if smoke.section_count > 0 {
                smoke_trail_list.push(smoke);
            }
        }

        return smoke_trail_list;
    }

    pub fn import_tracer_multiple(buf: &[u8], uv_idx: usize, scale: f32) -> Vec<Self> {
        let (tracer_list, []) = buf.as_chunks::<TRACER_TRAIL_SIZE>() else {
            panic!("buf length not a multiple of TRACER_TRAIL_SIZE");
        };

        let mut tracer_trail_list: Vec<Self> = Vec::with_capacity(tracer_list.len());
        for tracer_buf in tracer_list {
            let tracer = Self::import_tracer(tracer_buf, uv_idx, scale);
            if tracer.section_count > 0 {
                tracer_trail_list.push(tracer);
            }
        }

        return tracer_trail_list;
    }

    pub fn export(&self, path: &Path, uv_path_fn: FnIndexToPath) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let uv_path = uv_path_fn(self.uv_idx);
        write_godot_path(&uv_path, &mut writer)?;

        for c in self.color_start.to_array() {
            writer.write_u8(c)?;
        }

        for c in self.color_max.to_array() {
            writer.write_u8(c)?;
        }

        writer.write_u8(self.section_count)?;
        writer.write_f32::<LittleEndian>(self.texture_scale)?;
        writer.write_f32::<LittleEndian>(self.width_start)?;
        writer.write_f32::<LittleEndian>(self.width_end)?;
        writer.write_u32::<LittleEndian>(self.flags)?;

        return Ok(());
    }
}

impl fmt::Display for TrailEffect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "UV idx: {}, Color Start: {}, Color Max: {}, Section Count: {}, Texture Scale: {}, Width Start: {}, Width End: {}",
            self.uv_idx,
            self.color_start,
            self.color_max,
            self.section_count,
            self.texture_scale,
            self.width_start,
            self.width_end,
        )
    }
}
