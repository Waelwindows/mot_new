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
    pub fn to_raw(self, mot_db: &MotionSetDatabase) -> Result<RawMotion, UnqualifyMotionError> {
        use std::array::IntoIter;
        let bones = self.anims.keys()
            .into_iter()
            .map(|x| {
                mot_db
                    .bones
                    .iter()
                    .position(|y| &x[..] == y)
                    .map(|y| y as u16)
                    .ok_or_else(|| UnqualifyMotionError::NotInDatabase(x.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut sets: Vec<_> = self.anims.values()
            .into_iter()
            .filter_map(|x| x.as_ref())
            .cloned()
            .map(BoneAnim::to_vec)
            .map(|x| x.into_iter().map(|(u, v, w)| IntoIter::new([u, v, w])))
            .flatten()
            .flatten()
            .collect();
        //HACK: Padding?
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
            let mode = match bone {
                Some(b) => Some(b.mode),
                None if name == "gblctr" => Some(BoneType::Position),
                None if name == "kg_ya_ex" =>  Some(BoneType::Rotation),
                None => None,
            };
            let anim = match mode {
                Some(BoneType::Rotation) => Some(BoneAnim::Rotation(vec3()?)),
                Some(BoneType::Type1) => Some(BoneAnim::Unk(vec3()?, vec3()?)),
                Some(BoneType::Position) => Some(BoneAnim::Position(vec3()?)),
                Some(BoneType::Type3) => Some(BoneAnim::PositionRotation {
                    position: vec3()?,
                    rotation: vec3()?,
                }),
                Some(BoneType::Type4) => Some(BoneAnim::RotationIk {
                    target: vec3()?,
                    rotation: vec3()?,
                }),
                Some(BoneType::Type5) => Some(BoneAnim::ArmIk {
                    target: vec3()?,
                    rotation: vec3()?,
                }),
                Some(BoneType::Type6) => Some(BoneAnim::LegIk {
                    target: vec3()?,
                    position: vec3()?,
                }),
                None => None,
            };
            anims.insert(Bone(name.clone()), anim);
        }
        dbg!(sets.len());
        Ok(Self {
            anims,
            frames: mot.frames,
        })
    }
}

impl<'a> core::ops::Deref for Bone<'a> {
    type Target = str;

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

        let unq = mot.clone().to_raw(&mot_db)?;
        assert_eq!(raw.sets, unq.sets);

        let mot = Motion::from_raw(unq.clone(), &mot_db, &bone_db)?;
        let last = mot.to_raw(&mot_db)?;

        assert_eq!(raw.sets, last.sets);
        Ok(())
    }
}
