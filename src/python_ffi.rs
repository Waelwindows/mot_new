use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use pyo3::PyObjectProtocol;

use super::*;

#[pyfunction]
fn read_raw_mot(path: String) -> PyResult<PyRawMotion> {
    use std::fs::File;
    use std::io::Read;

    let input = std::fs::read(path)?;
    let (_, raw) = RawMotion::parse(&input).unwrap();
    Ok(raw.into())
}

#[pyfunction]
fn read_mot(path: String, mot_db: String, bone_db: String) -> PyResult<PyMotion> {
    use std::io::Read;

    let input = std::fs::read(path)?;
    let (_, raw) = RawMotion::parse(&input).unwrap();

    let input = std::fs::read(mot_db)?;
    let (_, mot_db) = diva_db::mot::MotionSetDatabase::read(nom::number::Endianness::Little)(&input).unwrap();
    let input = std::fs::read(bone_db)?;
    let (_, bone_db) = diva_db::bone::BoneDatabase::read(&input).unwrap();

    let mot = Motion::from_raw(raw, &mot_db, &bone_db).unwrap();

    Ok(mot.into())
}

#[pymodule]
fn mot(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(read_raw_mot))?;
    m.add_wrapped(wrap_pyfunction!(read_mot))?;
    m.add_class::<PyRawMotion>()?;
    m.add_class::<PyMotion>()?;
    m.add_class::<PyBoneAnim>()?;
    m.add_class::<PyVec3>()?;
    m.add_class::<PyKeyframe>()?;

    Ok(())
}

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PyRawMotion {
    #[pyo3(get, set)]
    pub sets: Vec<PyKeySet>,
    #[pyo3(get, set)]
    pub bones: Vec<u16>,
    #[pyo3(get)]
    pub frames: u16,
}

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PyMotion {
    #[pyo3(get)]
    pub frames: u16,
    #[pyo3(get, set)]
    anims: BTreeMap<String, Option<PyBoneAnim>>,
}

pub type PyKeySet = Vec<PyKeyframe>;

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PyBoneAnim {
    #[pyo3(get, set)]
    position: Option<PyVec3>,
    #[pyo3(get, set)]
    rotation: Option<PyVec3>,
    #[pyo3(get, set)]
    target: Option<PyVec3>,
}

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PyVec3 {
    #[pyo3(get, set)]
    x: PyKeySet,
    #[pyo3(get, set)]
    y: PyKeySet,
    #[pyo3(get, set)]
    z: PyKeySet,
}

#[pyclass]
#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub struct PyKeyframe {
    #[pyo3(get, set)]
    pub frame: Option<u16>,
    #[pyo3(get, set)]
    pub value: f32,
    #[pyo3(get, set)]
    pub interpolation: Option<f32>,
}

impl From<RawMotion> for PyRawMotion {
    fn from(mot: RawMotion) -> Self {
        let sets = mot
            .sets
            .into_iter()
            .map(PyKeyframe::from_frame_data)
            .collect();
        let bones = mot.bones;
        let frames = mot.frames;
        Self { sets, bones, frames }
    }
}

impl<'a> From<Motion<'a>> for PyMotion {
    fn from(mot: Motion) -> Self {
        let anims = mot
            .anims
            .into_iter()
            .map(|(b, a)| (b.name[..].to_string(), a.map(|x| x.into())))
            .collect();
        Self { anims, frames: mot.frames }
    }
}

impl From<BoneAnim> for PyBoneAnim {
    fn from(anim: BoneAnim) -> Self {
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

impl From<Vec3> for PyVec3 {
    fn from((x, y, z): Vec3) -> Self {
        Self {
            x: PyKeyframe::from_frame_data(x),
            y: PyKeyframe::from_frame_data(y),
            z: PyKeyframe::from_frame_data(z),
        }
    }
}

impl PyKeyframe {
    fn from_frame_data(data: FrameData) -> PyKeySet {
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

impl From<Keyframe> for PyKeyframe {
    fn from(key: Keyframe) -> Self {
        Self {
            frame: Some(key.frame),
            value: key.value,
            interpolation: None,
        }
    }
}

impl From<Keyframe<Hermite>> for PyKeyframe {
    fn from(key: Keyframe<Hermite>) -> Self {
        Self {
            frame: Some(key.frame),
            value: key.value,
            interpolation: Some(key.interpolation),
        }
    }
}

#[pyproto]
impl<'p> PyObjectProtocol<'p> for PyRawMotion {
    fn __repr__(&'p self) -> PyResult<String> {
        Ok(format!(
            "PyRawMotion: {} frames, {} sets, {} bones",
            self.frames,
            self.sets.len(),
            self.bones.len(),
        ))
    }
}
#[pyproto]
impl<'p> PyObjectProtocol<'p> for PyMotion {
    fn __repr__(&'p self) -> PyResult<String> {
        Ok(format!("PyMotion: {} frames, {} bone animations", self.frames, self.anims.len(),))
    }
}
#[pyproto]
impl<'p> PyObjectProtocol<'p> for PyBoneAnim {
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
        Ok(format!("PyBoneAnim: {}", s))
    }
}
#[pyproto]
impl<'p> PyObjectProtocol<'p> for PyVec3 {
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
        Ok(format!("PyVec3: {}", s))
    }
}
#[pyproto]
impl<'p> PyObjectProtocol<'p> for PyKeyframe {
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
            "PyKeyframe({}value: {}{})",
            frame, self.value, interp
        ))
    }
}
