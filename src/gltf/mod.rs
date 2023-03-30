use std::path::Path;

use color_eyre::Result;

mod conversions;
pub use conversions::*;

pub struct GltfDocument {
    pub document: gltf::Document,
    pub buffers: Vec<gltf::buffer::Data>,
    pub images: Vec<gltf::image::Data>,
}

impl GltfDocument {
    pub fn import(path: impl AsRef<Path>) -> Result<Self> {
        let (document, buffers, images) = gltf::import(path)?;
        Ok(Self {
            document,
            buffers,
            images,
        })
    }

    pub fn data_of_accessor<'a>(&'a self, accessor: &gltf::Accessor<'a>) -> Option<&'a [u8]> {
        let buffer_view = accessor.view()?;
        let buffer = buffer_view.buffer();
        let buffer_data = &self.buffers[buffer.index()];
        let buffer_view_data =
            &buffer_data[buffer_view.offset()..buffer_view.offset() + buffer_view.length()];
        let accessor_data = &buffer_view_data
            [accessor.offset()..accessor.offset() + accessor.count() * accessor.size()];
        Some(accessor_data)
    }
}
