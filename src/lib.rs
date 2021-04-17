mod read;

pub struct RawMotion {
    sets: Vec<FrameData>,
    bones: Vec<u16>,
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
pub struct Keyframe<I=()> {
    pub frame: u16,
    pub value: f32,
    pub interpolation: I,
}
