use super::*;
use crate::read::SetType;

use std::io;

impl RawMotion {
    fn write<W: io::Write + io::Seek>(&self, mut w: W) -> io::Result<(usize, usize)> {
        use std::io::{Seek, SeekFrom};
        w.write_all(&(self.sets.len() as u16).to_le_bytes())?;
        w.write_all(&self.frames.to_le_bytes())?;
        let set_ty: Vec<SetType> = self.sets.iter().map(From::from).collect();
        let mut set_ty_bytes = SetType::as_bytes(&set_ty);
        let start = w.seek(SeekFrom::Current(0))? as usize;
        set_ty_bytes.append(&mut vec![0u8; (start + set_ty_bytes.len()) % 4]);
        w.write_all(&set_ty_bytes)?;
        let set_off= w.seek(SeekFrom::Current(0))? as usize;
        let mut written_sets = 0;
        for set in &self.sets {
            written_sets += set.write(&mut w)?;
        }
        let bone_off= w.seek(SeekFrom::Current(0))? as usize;
        for bone in &self.bones {
            w.write_all(&bone.to_le_bytes())?;
        }
        w.write_all(&[0; 2])?;
        Ok((set_off, bone_off))
    }
    pub fn write_all<W: io::Write + io::Seek>(mots: &[Self], mut w: W) -> io::Result<()> {
        use std::io::{Seek, SeekFrom};
        let header_size = (1 + mots.len()) * 16;
        w.write_all(&vec![0; header_size])?;
        let mut infos = vec![];
        for mot in mots {
            //Get stream position
            let start = w.seek(SeekFrom::Current(0))?;
            
            let (set, bone) = mot.write(&mut w)?;
            infos.push((start, set, bone));
        }
        w.seek(SeekFrom::Start(0))?;
        for (start, set, bone) in infos {
            let start = start as u32;
            let info = start + 4;
            let set = set as u32;
            let bone = bone as u32;
            w.write_all(&start.to_le_bytes())?;
            w.write_all(&info.to_le_bytes())?;
            w.write_all(&set.to_le_bytes())?;
            w.write_all(&bone.to_le_bytes())?;
        }
        w.write_all(&[0u8; 16])?;
        Ok(())
    }
}

impl SetType {
    fn as_byte(ty: &[Self]) -> u8 {
        let b = |x: usize| *ty.get(x).unwrap_or(&SetType::None) as u8;
        let bytes = [b(0), b(1), b(2), b(3)];
        let byte = bytes[0] | bytes[1] << 2 | bytes[2] << 4 | bytes[3] << 6;
        byte
    }
    fn as_bytes(tys: &[Self]) -> Vec<u8> {
        tys.chunks(4).map(SetType::as_byte).collect()
    }
}

impl From<&FrameData> for SetType {
    fn from(data: &FrameData) -> Self { match data {
            FrameData::None => SetType::None,
            FrameData::Pose(_) => SetType::Pose,
            FrameData::CatmulRom(_) => SetType::CatmullRom,
            FrameData::Hermite(_) => SetType::Hermite,
        }
    }
}

impl FrameData {
    fn write<W: io::Write + io::Seek>(&self, mut w: W) -> io::Result<usize> {
        use std::io::{Seek, SeekFrom};
        match self {
            FrameData::None => Ok(0),
            FrameData::Pose(p) => w.write_all(&p.to_le_bytes()).map(|_| 4),
            FrameData::CatmulRom(v) => {
                w.write_all(&(v.len() as u16).to_le_bytes())?;
                for frame in v {
                    w.write_all(&frame.frame.to_le_bytes())?;
                }
                let pos = w.seek(SeekFrom::Current(0))? as usize;
                w.write_all(&vec![0; pos % 4])?;
                for frame in v {
                    w.write_all(&frame.value.to_le_bytes())?;
                }
                Ok(2 + 6 * v.len())
            }
            FrameData::Hermite(v) => {
                w.write_all(&(v.len() as u16).to_le_bytes())?;
                for frame in v {
                    w.write_all(&frame.frame.to_le_bytes())?;
                }
                let pos = w.seek(SeekFrom::Current(0))? as usize;
                w.write_all(&vec![0; pos % 4])?;
                for frame in v {
                    w.write_all(&frame.value.to_le_bytes())?;
                    w.write_all(&frame.interpolation.to_le_bytes())?;
                }
                Ok(2 + 10 * v.len())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &'static [u8] = include_bytes!("../assets/mot_PV001.bin");

    #[test]
    fn same_output() -> anyhow::Result<()> {
        let raw = crate::RawMotion::read(INPUT)?;
        let mut out = std::io::Cursor::new(vec![]);
        crate::RawMotion::write_all(&raw, &mut out)?;
        let inner = out.into_inner();
        let der = crate::RawMotion::read(&inner)?;
        assert_eq!(raw, der);
        Ok(())
    }
}
