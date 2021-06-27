use diva_db::bone::BoneDatabase;
use diva_db::mot::MotionSetDatabase;
use thiserror::Error;

use std::collections::{BTreeMap, VecDeque};
use std::borrow::Cow;

mod ordering;
#[cfg(feature = "pyo3")]
pub mod python_ffi;
mod read;
mod write;
pub mod qualify;

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct RawMotion {
    sets: Vec<FrameData>,
    bones: Vec<u16>,
    frames: u16,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct Motion<'a> {
    frames: u16,
    pub anims: BTreeMap<Bone<'a>, Option<BoneAnim>>,
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct Bone<'a>(Cow<'a, str>);

type Vec3 = (FrameData, FrameData, FrameData);

#[derive(Clone, PartialEq, PartialOrd, Debug)]
pub enum BoneAnim {
    ///Corresponds to Type 0
    Rotation(Vec3),
    ///Corresponds to Type 1
    Unk(Vec3, Vec3),
    ///Corresponds to Type 2
    Position(Vec3),
    ///Corresponds to Type 3
    PositionRotation { position: Vec3, rotation: Vec3 },
    ///Corresponds to Type 4
    RotationIk { target: Vec3, rotation: Vec3 },
    ///Corresponds to Type 5
    ArmIk { target: Vec3, rotation: Vec3 },
    ///Corresponds to Type 6
    LegIk { position: Vec3, target: Vec3 },
}

#[derive(Clone, PartialEq, PartialOrd, Debug)]
pub enum FrameData {
    None,
    Pose(f32),
    CatmulRom(Vec<Keyframe>),
    Hermite(Vec<Keyframe<Hermite>>),
}

type Hermite = f32;

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct Keyframe<I = ()> {
    pub frame: u16,
    pub value: f32,
    pub interpolation: I,
}
