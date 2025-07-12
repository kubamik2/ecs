use crate::bitmap::Bitmap;

#[derive(Default, Clone, Copy)]
pub struct Access {
    pub immutable: Bitmap,
    pub mutable: Bitmap,
    pub mutable_count: u32
}
