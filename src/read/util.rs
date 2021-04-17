use thiserror::*;

use core::array::TryFromSliceError;

pub type PResult<'a, O, E> = Result<(&'a [u8], O), E>;

#[derive(Debug, Error)]
#[error("Unexpected EOF. Wanted to read {want} bytes, {left} bytes left")]
pub struct OutOfRange {
    want: usize,
    left: usize,
}

#[derive(Debug, Error)]
#[error("Unexpected EOF. Wanted to read at {at:#X} but leftover input has len {len:#X}")]
pub struct OobPointer {
    pub at: usize,
    pub len: usize,
}

fn input_array<const N: usize>(i: &[u8]) -> PResult<&[u8; N], OutOfRange> {
    use core::convert::{TryFrom, TryInto};
    i.get(N..).ok_or_else(|| OutOfRange { want: N, left: i.len() })?;
    let (i, r) = i.split_at(N);
    let i = i.try_into().unwrap();
    Ok((r, i))
}

macro_rules! parse_int { 
    ($ty:ty, $le:ident, $be:ident) => {

        pub fn $le(i: &[u8]) -> Result<(&[u8], $ty), OutOfRange> {
            let (r, i) = input_array(i)?;
            let val = <$ty>::from_le_bytes(*i);
            Ok((r, val))
        }

        pub fn $be(i: &[u8]) -> Result<(&[u8], $ty), OutOfRange> {
            let (r, i) = input_array(i)?;
            let val = <$ty>::from_be_bytes(*i);
            Ok((r, val))
        }

    };
}

parse_int!(u16, le_u16, be_u16);
parse_int!(u32, le_u32, be_u32);
parse_int!(u64, le_u64, be_u64);

parse_int!(i16, le_i16, be_i16);
parse_int!(i32, le_i32, be_i32);
parse_int!(i64, le_i64, be_i64);

parse_int!(f32, le_f32, be_f32);
parse_int!(f64, le_f64, be_f64);

#[derive(Debug, Error)]
pub enum ReadAtError<E: std::error::Error + 'static> {
    #[error("Out of range. Points at {0}, file ends at {1}")]
    OutOfRange(usize, usize),
    #[error(transparent)]
    InternalParserError(#[from] E),
}

pub fn read_at<F, O, E>(off: usize, f: F) -> impl Fn(&[u8]) -> PResult<O, ReadAtError<E>> 
where
    F: Fn(&[u8]) -> PResult<O, E>,
    E: std::error::Error,
{
    move |i0: &[u8]| {
        let i = i0.get(off..).ok_or_else(|| ReadAtError::OutOfRange(off, i0.len()))?;
        f(i).map_err(ReadAtError::InternalParserError)
    }
}

pub fn count<F, O, E>(count: usize, f: F) -> impl Fn(&[u8]) -> PResult<Vec<O>, E>
where
    F: Fn(&[u8]) -> PResult<O, E>,
{
    move |i0: &[u8]| {
        let mut res = Vec::with_capacity(count);
        let mut i = i0;
        for _ in 0..count {
            let (i1, o) = f(i)?;
            res.push(o);
            i = i1;
        }
        Ok((i, res))
    }
}

pub fn pair<F1, F2, O1, O2, E>(f1: F1, f2: F2) -> impl Fn(&[u8]) -> PResult<(O1, O2), E>
where
    F1: Fn(&[u8]) -> PResult<O1, E>,
    F2: Fn(&[u8]) -> PResult<O2, E>,
{
    move |i0: &[u8]| {
        let (i, v1) = f1(i0)?;
        let (i, v2) = f2(i)?;
        Ok((i, (v1, v2)))
    }
}

pub fn map<M, F, O, U, E>(f: F, m: M) -> impl Fn(&[u8]) -> PResult<U, E>
where
    F: Fn(&[u8]) -> PResult<O, E>,
    M: Fn(O) -> U,
{
    move |i: &[u8]| {
        f(i).map(|(i, x)| (i, m(x)))
    }
}

pub fn many_till_nth<F, O, E>(f: F, c: O, nth: usize) -> impl Fn(&[u8]) -> PResult<Vec<O>, E>
where
    F: Fn(&[u8]) -> PResult<O, E>,
    O: PartialEq,
{
    move |i0: &[u8]| {
        let mut res = vec![];
        let mut occurance = 0;
        let mut i = i0;
        loop {
            let (i1, r) = f(i)?;
            if r == c {
                if occurance < nth {
                    occurance += 1;
                } else {
                    break;
                }
            }
            i = i1;
            res.push(r);
        }
        Ok((i0, res))
    }
}
