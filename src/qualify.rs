use super::*;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Error)]
pub enum MotionQualifyError {
    #[error("Found no skeleton in bone database")]
    NoSkeleton,
    #[error("Not enough sets")]
    PopSet,
    #[error("Bone id `{0}` not in motion database")]
    NotInMotDb(u16),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Error)]
pub enum UnqualifyMotionError {
    #[error("Bone `{0}` not found in motion database")]
    NotInDatabase(String),
}

impl BoneAnim {
    fn to_vec<'a>(self) -> Vec<Vec3> {
        match self {
            BoneAnim::Rotation(v) => vec![v],
            BoneAnim::Unk(u, v) => vec![u, v],
            BoneAnim::Position(v) => vec![v],
            BoneAnim::PositionRotation { position, rotation } => vec![position, rotation],
            BoneAnim::RotationIk { target, rotation } => vec![target, rotation],
            BoneAnim::ArmIk { target, rotation } => vec![target, rotation],
            BoneAnim::LegIk { position, target } => vec![target, position],
        }
    }
}

impl<'a> Motion<'a> {
    fn get_by_name(&self, name: &str) -> Option<&BoneAnim> {
        self.anims
            .iter()
            .filter_map(|(b, a)| a.as_ref().map(|x| (b, x)))
            .find(|(b, _)| b.name == name)
            .map(|(_, a)| a)
    }

    pub fn to_raw(self, mot_db: &MotionSetDatabase) -> Result<RawMotion, UnqualifyMotionError> {
        use std::array::IntoIter;
        let bones = self.anims.keys()
            .into_iter()
            .map(|x| {
                mot_db
                    .bones
                    .iter()
                    .position(|y| &x.name == y)
                    .map(|y| y as u16)
                    .ok_or_else(|| UnqualifyMotionError::NotInDatabase(x.name.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sets = self.anims.values()
            .into_iter()
            .filter_map(|x| x.as_ref())
            .cloned()
            .map(BoneAnim::to_vec)
            .map(|x| x.into_iter().map(|(u, v, w)| IntoIter::new([u, v, w])))
            .flatten()
            .flatten()
            .collect();
        Ok(RawMotion {
            bones,
            sets,
            frames: 0,
        })
    }

    pub fn from_raw(
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
                    #[cfg(feature = "tracing")]
                    {
                        use tracing::*;
                        error!(
                            "Bone `{}` not found in bone database, setting default",
                            name
                        );
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
        Ok(Self {
            anims,
            frames: mot.frames,
        })
    }
}

impl<'a> core::ops::Deref for Bone<'a> {
    type Target = diva_db::bone::Bone<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &'static [u8] = include_bytes!("../assets/mot_PV001.bin");
    const MOT_DB: &'static [u8] = include_bytes!("../assets/mot_db.bin");
    const BONE_DB: &'static [u8] = include_bytes!("../assets/bone_data.bin");

    #[test]
    fn roundtrip() -> anyhow::Result<()> {
        let mut raw = crate::RawMotion::read(INPUT)?.remove(0);
        let mot_db = diva_db::mot::MotionSetDatabase::read(MOT_DB)?.1;
        let bone_db = diva_db::bone::BoneDatabase::read(BONE_DB)?.1;
        let mot = Motion::from_raw(raw.clone(), &mot_db, &bone_db)?;
        dbg!(raw.bones.len(), mot.anims.len());
        use BoneAnim::*;
        let sum = mot
            .anims
            .values()
            .filter_map(Option::as_ref)
            .map(|x| match x {
                Rotation(_) | Position(_) => 1,
                Unk(_, _)
                | PositionRotation { .. }
                | RotationIk { .. }
                | ArmIk { .. }
                | LegIk { .. } => 2,
            })
            .sum::<usize>();
        dbg!(sum);
        dbg!(raw.sets.len());
        let unq = mot.clone().to_raw(&mot_db)?;
        dbg!(unq.bones.len());
        let mot2 = Motion::from_raw(unq.clone(), &mot_db, &bone_db)?;
        let mut file = std::fs::File::create("out.mot")?;
        RawMotion::write_all(&[unq.clone()], &mut file)?;
        assert_eq!(raw.sets.len(), unq.sets.len());
        Ok(())
    }
}
