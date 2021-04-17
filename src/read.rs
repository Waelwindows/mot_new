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
    SetTypeReadError(#[from] #[source] ReadAtError<Infallible>),
    #[error("Failed to read frame data at {0} pos")]
    FrameReadError(usize, #[source] OutOfRange)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SetType {
    None,
    Pose,
    CatmullRom,
    Hermite,
}

impl RawMotion {
    pub fn parse(i0: &[u8]) -> PResult<Self, RawMotionError> {
        let (i, info_off) = le_u32(i0)?;
        let (i, set_ty_off) = le_u32(i)?;
        let (i, set_off) = le_u32(i)?;
        let (i, bone_off) = le_u32(i)?;

        let (_, info) = read_at(info_off as usize, le_u32)(i0)?;
        let cnt = info as usize & 0x3FFF;

        //Must divide by 4 as `SetType::parse` reads 4 types at a time
        let cnt1 = (cnt as f32 / 4.).ceil() as usize;

        let (_, set_ty) = read_at(
            set_ty_off as usize,
            count(cnt1, SetType::parse_helper),
        )(i0)?;

        let mut sets = Vec::with_capacity(cnt);
        let mut i = i0.get(set_off as usize ..).ok_or_else(|| OobPointer { at: set_off as usize, len: i0.len()  })?;
        for (j, ty) in set_ty.iter().flatten().enumerate().take(cnt) {
            let (i1, v) = FrameData::parse(*ty)(i).map_err(|e| RawMotionError::FrameReadError(j, e))?;
            i = i1;
            sets.push(v);
        }

        let (i, bones) = read_at(bone_off as usize, many_till_nth(le_u16, 0, 1))(i0)?;

        Ok((i, Self { sets, bones }))
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
        move |i: &[u8]| {
            match ty {
                SetType::None => Ok((i, Self::None)),
                SetType::Pose => map(le_f32, Self::Pose)(i),
                SetType::CatmullRom => map(Keyframe::<()>::parse, Self::CatmulRom)(i),
                SetType::Hermite => map(Keyframe::<f32>::parse, Self::Hermite)(i),
            }
        }
    }
}

impl Keyframe {
    pub fn parse(i0: &[u8]) -> PResult<Vec<Self>, OutOfRange> {
        let (i, cnt) = le_u32(i0)?;
        let (i, frames) = count(cnt as usize, le_u16)(i)?;
        //Align at 4th byte
        let i = &i[i.len() % 4..];
        let (i, values) = count(cnt as usize, le_f32)(i)?;
        let keyframes = frames
            .into_iter()
            .zip(values.into_iter())
            .map(|(frame, value)| Self { frame, value, interpolation: () })
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
            .map(|(frame, (value, interpolation))| Self { frame, value, interpolation })
            .collect();
        Ok((i, keyframes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &[u8] = include_bytes!("../assets/mot_PV001.bin");

    #[test]
    fn test_raw_motion() {
        let mot = RawMotion::parse(INPUT).unwrap();
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
