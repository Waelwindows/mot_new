use diva_db::bone::BoneDatabase;
use diva_db::mot::MotionSetDatabase;
use thiserror::Error;

use std::collections::{BTreeMap, VecDeque};

mod ordering;
mod read;
#[cfg(feature="pyo3")]
pub mod python_ffi;

pub struct RawMotion {
    sets: Vec<FrameData>,
    bones: Vec<u16>,
}

pub struct Motion<'a> {
    anims: BTreeMap<Bone<'a>, Option<BoneAnim>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Bone<'a>(diva_db::bone::Bone<'a>);

type Vec3 = (FrameData, FrameData, FrameData);

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum FrameData {
    None,
    Pose(f32),
    CatmulRom(Vec<Keyframe>),
    Hermite(Vec<Keyframe<Hermite>>),
}

type Hermite = f32;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Keyframe<I = ()> {
    pub frame: u16,
    pub value: f32,
    pub interpolation: I,
}

#[derive(Debug, Error)]
pub enum MotionQualifyError {
    #[error("Found no skeleton in bone database")]
    NoSkeleton,
    #[error("Not enough sets")]
    PopSet,
    #[error("Bone id `{0}` not in motion database")]
    NotInMotDb(u16),
}

impl<'a> Motion<'a> {
    fn get_by_name(&self, name: &str) -> Option<&BoneAnim> {
        self.anims
            .iter()
            .filter_map(|(b, a)| a.as_ref().map(|x| (b, x)))
            .find(|(b, _)| b.name == name)
            .map(|(_, a)| a)
    }

    fn from_raw(
        mot: RawMotion,
        mot_db: &MotionSetDatabase<'a>,
        bone_db: &BoneDatabase<'a>,
    ) -> Result<Self, MotionQualifyError> {
        use diva_db::bone::BoneType;
        use MotionQualifyError::*;

        let mut sets: VecDeque<_> = mot.sets.into();
        let bones = &bone_db.skeletons.get(0).ok_or(NoSkeleton)?.bones;
        let mut vec3 = || -> Result<Vec3, MotionQualifyError> {
            let x = sets.pop_front().ok_or(PopSet)?;
            let y = sets.pop_front().ok_or(PopSet)?;
            let z = sets.pop_front().ok_or(PopSet)?;
            Ok((x, y, z))
        };
        let mut anims = BTreeMap::new();
        for id in mot.bones {
            let name = mot_db
                .bones
                .get(id as usize)
                .ok_or_else(|| NotInMotDb(id))?;
            let bone = bones.iter().find(|x| x.name == *name);
            let bone = match bone {
                Some(b) => b.clone(),
                None if name == "gblctr" => diva_db::bone::Bone {
                    mode: BoneType::Position,
                    name: "gblctr".into(),
                    ..Default::default()
                },
                None if name == "kg_ya_ex" => diva_db::bone::Bone {
                    mode: BoneType::Rotation,
                    name: "kg_ya_ex".into(),
                    ..Default::default()
                },
                None => {
                    let bone = diva_db::bone::Bone {
                        name: name.clone(),
                        ..Default::default()
                    };
                    anims.insert(Bone(bone), None);
                    #[cfg(feature="tracing")]
                    {
                        use tracing::*;
                        error!("Bone `{}` not found in bone database, setting default", name);
                    }
                    continue;
                }
            };
            let anim = match bone.mode {
                BoneType::Rotation => BoneAnim::Rotation(vec3()?),
                BoneType::Type1 => BoneAnim::Unk(vec3()?, vec3()?),
                BoneType::Position => BoneAnim::Position(vec3()?),
                BoneType::Type3 => BoneAnim::PositionRotation {
                    position: vec3()?,
                    rotation: vec3()?,
                },
                BoneType::Type4 => BoneAnim::RotationIk {
                    target: vec3()?,
                    rotation: vec3()?,
                },
                BoneType::Type5 => BoneAnim::ArmIk {
                    target: vec3()?,
                    rotation: vec3()?,
                },
                BoneType::Type6 => BoneAnim::LegIk {
                    target: vec3()?,
                    position: vec3()?,
                },
            };
            anims.insert(Bone(bone), Some(anim));
        }
        dbg!(sets.len());
        Ok(Self { anims })
    }
}

impl<'a> core::ops::Deref for Bone<'a> {
    type Target = diva_db::bone::Bone<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
