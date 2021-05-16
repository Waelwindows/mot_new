use thiserror::*;

use core::array::TryFromSliceError;
use core::convert::Infallible;

use super::*;

mod util;
use util::*;

#[derive(Debug, Error)]
pub enum RawMotionError {
    #[error(transparent)]
    ReadAtError(#[from] ReadAtError<OutOfRange>),
    #[error(transparent)]
    OutOfRange(#[from] OutOfRange),
    #[error(transparent)]
    OobPointer(#[from] OobPointer),
    #[error("Unexpected EOF. Not enough bytes to read set types")]
    SetTypeReadError(
        #[from]
        #[source]
        ReadAtError<Infallible>,
    ),
    #[error("Failed to read the {0}th frame data at {1}")]
    FrameReadError(usize, usize, #[source] OutOfRange),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SetType {
    None,
    Pose,
    CatmullRom,
    Hermite,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct HeaderOffsets {
    info: usize,
    set_types: usize,
    sets: usize,
    bones: usize,
}

impl HeaderOffsets {
    fn parse(i: &[u8]) -> PResult<Option<Self>, OutOfRange> {
        use core::convert::TryInto;

        let (i, info) = le_u32(i)?;
        let (i, set_types) = le_u32(i)?;
        let (i, sets) = le_u32(i)?;
        let (i, bones) = le_u32(i)?;
        if info == 0 && set_types == 0 && sets == 0 && bones == 0 {
            return Ok((i, None));
        }
        //panic if the values don't fit within a pointer
        let info = info.try_into().unwrap();
        let set_types = set_types.try_into().unwrap();
        let sets = sets.try_into().unwrap();
        let bones = bones.try_into().unwrap();
        Ok((
            i,
            Some(Self {
                info,
                set_types,
                sets,
                bones,
            }),
        ))
    }
}

impl RawMotion {
    pub fn read(i0: &[u8]) -> Result<Vec<Self>, RawMotionError> {
        let (_, headers) = many_till_nth(HeaderOffsets::parse, None, 0)(i0)?;
        let mut vec = Vec::with_capacity(headers.len());
        for header in headers {
            let (_, val) = Self::parse(i0, header.unwrap())?;
            vec.push(val);
        }
        Ok(vec)
    }
    fn parse(i0: &[u8], offsets: HeaderOffsets) -> PResult<Self, RawMotionError> {
        let (_, (info, frames)) = read_at(offsets.info, pair(le_u16, le_u16))(i0)?;
        let cnt = info as usize & 0x3FFF;

        dbg!(cnt);

        //Must divide by 4 as `SetType::parse` reads 4 types at a time
        let cnt1 = (cnt as f32 / 4.).ceil() as usize;

        let (_, set_ty) = read_at(offsets.set_types, count(cnt1, SetType::parse_helper))(i0)?;

        let mut sets = Vec::with_capacity(cnt);
        let mut i = i0.get(offsets.sets..).ok_or_else(|| OobPointer {
            at: offsets.sets,
            len: i0.len(),
        })?;
        for (j, ty) in set_ty.iter().flatten().enumerate().take(cnt) {
            let (i1, v) = FrameData::parse(*ty)(i)
                .map_err(|e| RawMotionError::FrameReadError(j, i0.len() - i.len(), e))?;
            i = i1;
            sets.push(v);
        }

        let (i, bones) = read_at(offsets.bones, many_till_nth(le_u16, 0, 1))(i0)?;

        Ok((
            i,
            Self {
                sets,
                bones,
                frames,
            },
        ))
    }
}

impl SetType {
    fn from_bits(i: u8) -> Option<Self> {
        match i {
            0 => Some(SetType::None),
            1 => Some(SetType::Pose),
            2 => Some(SetType::CatmullRom),
            3 => Some(SetType::Hermite),
            _ => None,
        }
    }
    fn parse(i: u8) -> [Self; 4] {
        let i0 = (i & 0b0000_0011) >> 0;
        let i1 = (i & 0b0000_1100) >> 2;
        let i2 = (i & 0b0011_0000) >> 4;
        let i3 = (i & 0b1100_0000) >> 6;

        //the bitshifts guarrantee that values are in 0..4
        let v0 = Self::from_bits(i0).unwrap();
        let v1 = Self::from_bits(i1).unwrap();
        let v2 = Self::from_bits(i2).unwrap();
        let v3 = Self::from_bits(i3).unwrap();

        [v0, v1, v2, v3]
    }

    fn parse_helper(i: &[u8]) -> PResult<[Self; 4], Infallible> {
        Ok((&i[1..], Self::parse(i[0])))
    }
}

impl FrameData {
    pub fn parse(ty: SetType) -> impl Fn(&[u8]) -> PResult<Self, OutOfRange> {
        move |i: &[u8]| match ty {
            SetType::None => Ok((i, Self::None)),
            SetType::Pose => map(le_f32, Self::Pose)(i),
            SetType::CatmullRom => map(Keyframe::<()>::parse, Self::CatmulRom)(i),
            SetType::Hermite => map(Keyframe::<f32>::parse, Self::Hermite)(i),
        }
    }
}

impl Keyframe {
    pub fn parse(i0: &[u8]) -> PResult<Vec<Self>, OutOfRange> {
        let (i, cnt) = le_u16(i0)?;
        let (i, frames) = count(cnt as usize, le_u16)(i)?;
        //Align at 4th byte
        let i = &i[i.len() % 4..];
        let (i, values) = count(cnt as usize, le_f32)(i)?;
        let keyframes = frames
            .into_iter()
            .zip(values.into_iter())
            .map(|(frame, value)| Self {
                frame,
                value,
                interpolation: (),
            })
            .collect();
        Ok((i, keyframes))
    }
}

impl Keyframe<Hermite> {
    pub fn parse(i0: &[u8]) -> PResult<Vec<Self>, OutOfRange> {
        let (i, cnt) = le_u16(i0)?;
        let (i, frames) = count(cnt as usize, le_u16)(i)?;
        //Align at 4th byte
        let i = &i[i.len() % 4..];
        let (i, values) = count(cnt as usize, pair(le_f32, le_f32))(i)?;
        let keyframes = frames
            .into_iter()
            .zip(values.into_iter())
            .map(|(frame, (value, interpolation))| Self {
                frame,
                value,
                interpolation,
            })
            .collect();
        Ok((i, keyframes))
    }
}

#[cfg(test)]
mod tests {
    use anyhow::*;

    use super::*;

    const INPUT: &[u8] = include_bytes!("../assets/mot_PV001.bin");
    const MOT_DB: &[u8] = include_bytes!("../../diva_db/assets/aft_mot_db.bin");
    const BONE_DB: &[u8] = include_bytes!("../../diva_db/assets/aft_bone_data.bin");

    #[test]
    fn test_raw_motion() -> Result<()> {
        let (_, header) = HeaderOffsets::parse(INPUT)?;
        let (i, mot) = RawMotion::parse(INPUT, header.unwrap())?;
        assert_eq!(i, &[]);
        assert_eq!(mot.sets.len(), 583);
        assert_eq!(mot.bones.len(), 193);
        assert_eq!(mot.frames, 9301);
        Ok(())
    }

    #[test]
    fn raw_motions() -> Result<()> {
        let mots = RawMotion::read(INPUT)?;
        assert_eq!(mots.len(), 1);
        Ok(())
    }

    #[test]
    fn qualify_motion() -> Result<()> {
        let (_, header) = HeaderOffsets::parse(INPUT)?;
        let (_, mot) = RawMotion::parse(INPUT, header.unwrap())?;
        let (_, motdb) = diva_db::mot::MotionSetDatabase::read(MOT_DB).unwrap();
        let (_, bonedb) = diva_db::bone::BoneDatabase::read(BONE_DB).unwrap();
        let qual = Motion::from_raw(mot, &motdb, &bonedb).unwrap();

        assert_eq!(qual.anims.len(), 192);
        Ok(())
    }

    #[test]
    fn parse_set_type() {
        use SetType::*;
        let i = &[0b11_10_01_00u8, 0b11_00_01_11][..];
        let val = SetType::parse(i[0]);
        let val1 = SetType::parse(i[1]);
        assert_eq!(
            [val, val1],
            [
                [None, Pose, CatmullRom, Hermite],
                [Hermite, Pose, None, Hermite]
            ]
        )
    }
}
