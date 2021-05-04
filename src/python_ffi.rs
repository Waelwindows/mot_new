use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use pyo3::PyObjectProtocol;

use std::collections::BTreeMap;

#[pyfunction]
fn read_raw_mot(path: String) -> PyResult<Vec<RawMotion>> {
    use super::*;

    use std::fs::File;
    use std::io::Read;

    let input = std::fs::read(path)?;
    let raw = RawMotion::read(&input).unwrap();
    Ok(raw.into_iter().map(Into::into).collect())
}

#[pyfunction]
fn read_mot(path: String, mot_db: String, bone_db: String) -> PyResult<Vec<Motion>> {
    use super::*;
    use std::io::Read;

    let input = std::fs::read(path)?;
    let raws = RawMotion::read(&input).unwrap();

    let input = std::fs::read(mot_db)?;
    let (_, mot_db) = diva_db::mot::MotionSetDatabase::read(&input).unwrap();
    let input = std::fs::read(bone_db)?;
    let (_, bone_db) = diva_db::bone::BoneDatabase::read(&input).unwrap();

    let mots = raws
        .into_iter()
        .map(|x| super::Motion::from_raw(x, &mot_db, &bone_db).map(From::from))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    Ok(mots)
}

#[pymodule]
fn mot(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(read_raw_mot))?;
    m.add_wrapped(wrap_pyfunction!(read_mot))?;
    m.add_class::<RawMotion>()?;
    m.add_class::<Motion>()?;
    m.add_class::<BoneAnim>()?;
    m.add_class::<Vec3>()?;
    m.add_class::<Keyframe>()?;

    Ok(())
}

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RawMotion {
    #[pyo3(get, set)]
    pub sets: Vec<KeySet>,
    #[pyo3(get, set)]
    pub bones: Vec<u16>,
    #[pyo3(get)]
    pub frames: u16,
}

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Motion {
    #[pyo3(get)]
    pub frames: u16,
    #[pyo3(get, set)]
    anims: BTreeMap<String, Option<BoneAnim>>,
}

pub type KeySet = Vec<Keyframe>;

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BoneAnim {
    #[pyo3(get, set)]
    position: Option<Vec3>,
    #[pyo3(get, set)]
    rotation: Option<Vec3>,
    #[pyo3(get, set)]
    target: Option<Vec3>,
}

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Vec3 {
    #[pyo3(get, set)]
    x: KeySet,
    #[pyo3(get, set)]
    y: KeySet,
    #[pyo3(get, set)]
    z: KeySet,
}

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub struct Keyframe {
    #[pyo3(get, set)]
    pub frame: Option<u16>,
    #[pyo3(get, set)]
    pub value: f32,
    #[pyo3(get, set)]
    pub interpolation: Option<f32>,
}

impl From<super::RawMotion> for self::RawMotion {
    fn from(mot: super::RawMotion) -> Self {
        let sets = mot
            .sets
            .into_iter()
            .map(Keyframe::from_frame_data)
            .collect();
        let bones = mot.bones;
        let frames = mot.frames;
        Self {
            sets,
            bones,
            frames,
        }
    }
}

impl<'a> From<super::Motion<'a>> for self::Motion {
    fn from(mot: super::Motion) -> Self {
        let anims = mot
            .anims
            .into_iter()
            .map(|(b, a)| (b.name[..].to_string(), a.map(|x| x.into())))
            .collect();
        Self {
            anims,
            frames: mot.frames,
        }
    }
}

impl From<super::BoneAnim> for self::BoneAnim {
    fn from(anim: super::BoneAnim) -> Self {
        use super::*;
        match anim {
            BoneAnim::Rotation(r) => Self {
                rotation: Some(r.into()),
                ..Default::default()
            },
            BoneAnim::Unk(_, _) => todo!("Implement converting TYPE 1 into python"),
            BoneAnim::Position(r) => Self {
                position: Some(r.into()),
                ..Default::default()
            },
            BoneAnim::PositionRotation { position, rotation } => Self {
                position: Some(position.into()),
                rotation: Some(rotation.into()),
                ..Default::default()
            },
            BoneAnim::RotationIk { target, rotation } => Self {
                target: Some(target.into()),
                rotation: Some(rotation.into()),
                ..Default::default()
            },
            BoneAnim::ArmIk { target, rotation } => Self {
                target: Some(target.into()),
                rotation: Some(rotation.into()),
                ..Default::default()
            },
            BoneAnim::LegIk { target, position } => Self {
                target: Some(target.into()),
                position: Some(position.into()),
                ..Default::default()
            },
        }
    }
}

impl From<super::Vec3> for self::Vec3 {
    fn from((x, y, z): super::Vec3) -> Self {
        Self {
            x: Keyframe::from_frame_data(x),
            y: Keyframe::from_frame_data(y),
            z: Keyframe::from_frame_data(z),
        }
    }
}

impl Keyframe {
    fn from_frame_data(data: super::FrameData) -> KeySet {
        use super::*;
        match data {
            FrameData::None => vec![],
            FrameData::Pose(value) => vec![Self {
                value,
                ..Default::default()
            }],
            FrameData::CatmulRom(v) => v.into_iter().map(Into::into).collect(),
            FrameData::Hermite(v) => v.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<super::Keyframe> for self::Keyframe {
    fn from(key: super::Keyframe) -> Self {
        Self {
            frame: Some(key.frame),
            value: key.value,
            interpolation: None,
        }
    }
}

impl From<super::Keyframe<super::Hermite>> for self::Keyframe {
    fn from(key: super::Keyframe<super::Hermite>) -> Self {
        Self {
            frame: Some(key.frame),
            value: key.value,
            interpolation: Some(key.interpolation),
        }
    }
}

#[pyproto]
impl<'p> PyObjectProtocol<'p> for RawMotion {
    fn __repr__(&'p self) -> PyResult<String> {
        Ok(format!(
            "RawMotion: {} frames, {} sets, {} bones",
            self.frames,
            self.sets.len(),
            self.bones.len(),
        ))
    }
}
#[pyproto]
impl<'p> PyObjectProtocol<'p> for Motion {
    fn __repr__(&'p self) -> PyResult<String> {
        Ok(format!(
            "Motion: {} frames, {} bone animations",
            self.frames,
            self.anims.len(),
        ))
    }
}
#[pyproto]
impl<'p> PyObjectProtocol<'p> for BoneAnim {
    fn __repr__(&'p self) -> PyResult<String> {
        let mut cap = vec![];
        match self.position {
            Some(_) => cap.push("position"),
            _ => {}
        };
        match self.rotation {
            Some(_) => cap.push("rotation "),
            _ => {}
        };
        match self.target {
            Some(_) => cap.push("target"),
            _ => {}
        };
        let mut s = cap.join(", ");
        if s == "" {
            s += "empty";
        }
        Ok(format!("BoneAnim: {}", s))
    }
}
#[pyproto]
impl<'p> PyObjectProtocol<'p> for Vec3 {
    fn __repr__(&'p self) -> PyResult<String> {
        let mut cap = vec![];
        if self.x.len() != 0 {
            cap.push("x");
        }
        if self.y.len() != 0 {
            cap.push("y");
        }
        if self.z.len() != 0 {
            cap.push("z");
        }
        let mut s = cap.join(" ");
        if s == "" {
            s += "empty";
        }
        Ok(format!("Vec3: {}", s))
    }
}
#[pyproto]
impl<'p> PyObjectProtocol<'p> for Keyframe {
    fn __repr__(&'p self) -> PyResult<String> {
        let frame = match self.frame {
            Some(p) => format!("frame: {}, ", p),
            _ => "".into(),
        };
        let interp = match self.interpolation {
            Some(p) => format!(", interp: {}", p),
            _ => "".into(),
        };
        Ok(format!(
            "Keyframe({}value: {}{})",
            frame, self.value, interp
        ))
    }
}
