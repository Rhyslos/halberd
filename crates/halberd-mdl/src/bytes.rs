//! Reading numbers and names at offsets in a file, with every read checked.

use crate::MdlError;

/// A file's bytes, read at offsets. Every read is checked against the
/// file's length; a read past the end is reported as damage in `file`.
#[derive(Clone, Copy)]
pub(crate) struct Bytes<'a> {
    pub(crate) data: &'a [u8],
    pub(crate) file: &'static str,
}

impl<'a> Bytes<'a> {
    pub(crate) fn new(data: &'a [u8], file: &'static str) -> Self {
        Self { data, file }
    }

    /// `n` bytes at `at`.
    pub(crate) fn slice(&self, at: usize, n: usize) -> Result<&'a [u8], MdlError> {
        at.checked_add(n)
            .and_then(|end| self.data.get(at..end))
            .ok_or(MdlError::Damaged(self.file, "something points past its end"))
    }

    fn array<const N: usize>(&self, at: usize) -> Result<[u8; N], MdlError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.slice(at, N)?);
        Ok(out)
    }

    pub(crate) fn u16(&self, at: usize) -> Result<u16, MdlError> {
        Ok(u16::from_le_bytes(self.array(at)?))
    }

    pub(crate) fn i16(&self, at: usize) -> Result<i16, MdlError> {
        Ok(i16::from_le_bytes(self.array(at)?))
    }

    pub(crate) fn i32(&self, at: usize) -> Result<i32, MdlError> {
        Ok(i32::from_le_bytes(self.array(at)?))
    }

    pub(crate) fn f32(&self, at: usize) -> Result<f32, MdlError> {
        Ok(f32::from_le_bytes(self.array(at)?))
    }

    pub(crate) fn vec3(&self, at: usize) -> Result<[f32; 3], MdlError> {
        Ok([self.f32(at)?, self.f32(at + 4)?, self.f32(at + 8)?])
    }

    /// A count: an `i32` that must be between 0 and `max`.
    pub(crate) fn count(&self, at: usize, max: usize, what: &'static str) -> Result<usize, MdlError> {
        let n = self.i32(at)?;
        let n = usize::try_from(n).map_err(|_| MdlError::Damaged(self.file, what))?;
        if n > max {
            return Err(MdlError::TooLarge(what));
        }
        Ok(n)
    }

    /// `base` moved by the signed offset stored at `at` (offsets in model
    /// files are relative to the structure that holds them).
    pub(crate) fn offset(&self, base: usize, at: usize) -> Result<usize, MdlError> {
        let delta = i64::from(self.i32(at)?);
        let target = i64::try_from(base).unwrap_or(i64::MAX).saturating_add(delta);
        usize::try_from(target)
            .ok()
            .filter(|&t| t <= self.data.len())
            .ok_or(MdlError::Damaged(self.file, "something points outside it"))
    }

    /// A zero-terminated name at `at` (at most 512 bytes; odd bytes are
    /// replaced).
    pub(crate) fn name(&self, at: usize) -> Result<String, MdlError> {
        let rest = self
            .data
            .get(at..)
            .ok_or(MdlError::Damaged(self.file, "a name is outside it"))?;
        let end = rest
            .iter()
            .take(513)
            .position(|&b| b == 0)
            .ok_or(MdlError::Damaged(self.file, "a name is too long or never ends"))?;
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }

    /// A fixed-size name field of `len` bytes at `at`, up to its first zero.
    pub(crate) fn fixed_name(&self, at: usize, len: usize) -> Result<String, MdlError> {
        let field = self.slice(at, len)?;
        let end = field.iter().position(|&b| b == 0).unwrap_or(len);
        Ok(String::from_utf8_lossy(&field[..end]).into_owned())
    }
}
