use std::path::Path;

use color_eyre::Result;
use naga::{
    back::spv::{self, BindingMap},
    front::wgsl,
    valid::{Capabilities, ValidationError, ValidationFlags, Validator},
};

pub struct ShaderCompiler {
    parser: wgsl::Parser,
    validator: Validator,
    writer: spv::Writer,
}

impl ShaderCompiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_shader_module(&mut self, path: &Path) -> Result<Vec<u32>, CompilerError> {
        let source = std::fs::read_to_string(path)
            .map_err(|err| CompilerError::Read((err, path.display().to_string())))?;
        let module = self
            .parser
            .parse(&source)
            .map_err(|error| CompilerError::Compile { error, source })?;
        let module_info = self.validator.validate(&module)?;
        let mut words = vec![];
        self.writer.write(&module, &module_info, None, &mut words)?;
        Ok(words)
    }
}

impl Default for ShaderCompiler {
    fn default() -> Self {
        let parser = wgsl::Parser::new();
        let validator = Validator::new(ValidationFlags::all(), Capabilities::all());
        let options = get_options();
        let writer = spv::Writer::new(&options).unwrap();
        Self {
            parser,
            validator,
            writer,
        }
    }
}

// https://github.com/gfx-rs/wgpu/blob/master/wgpu-hal/src/vulkan/adapter.rs#L1166
fn get_options() -> spv::Options {
    let capabilities = vec![
        spv::Capability::Shader,
        spv::Capability::Matrix,
        spv::Capability::Sampled1D,
        spv::Capability::Image1D,
        spv::Capability::ImageQuery,
        spv::Capability::DerivativeControl,
        spv::Capability::SampledCubeArray,
        spv::Capability::SampleRateShading,
        //Note: this is requested always, no matter what the actual
        // adapter supports. It's not the responsibility of SPV-out
        // translation to handle the storage support for formats.
        spv::Capability::StorageImageExtendedFormats,
        spv::Capability::MultiView,
        // TODO: fill out the rest
        spv::Capability::ImageBasic,
        spv::Capability::ImageReadWrite,
    ];

    let mut flags = spv::WriterFlags::empty();
    flags.set(
        spv::WriterFlags::DEBUG,
        true,
        // self.instance.flags.contains(crate::InstanceFlags::DEBUG),
    );
    flags.set(
        spv::WriterFlags::LABEL_VARYINGS,
        true, // self.phd_capabilities.properties.vendor_id != crate::auxil::db::qualcomm::VENDOR,
    );
    flags.set(
        spv::WriterFlags::FORCE_POINT_SIZE,
        //Note: we could technically disable this when we are compiling separate entry points,
        // and we know exactly that the primitive topology is not `PointList`.
        // But this requires cloning the `spv::Options` struct, which has heap allocations.
        true, // could check `super::Workarounds::SEPARATE_ENTRY_POINTS`
    );
    spv::Options {
        binding_map: BindingMap::new(),
        lang_version: (1, 0),
        flags,
        capabilities: Some(capabilities.into_iter().collect()),
        zero_initialize_workgroup_memory: spv::ZeroInitializeWorkgroupMemoryMode::Native,
        bounds_check_policies: naga::proc::BoundsCheckPolicies {
            index: naga::proc::BoundsCheckPolicy::Unchecked,
            buffer: naga::proc::BoundsCheckPolicy::Unchecked,
            image: naga::proc::BoundsCheckPolicy::Unchecked,
            binding_array: naga::proc::BoundsCheckPolicy::Unchecked,
        },
    }
}

pub enum CompilerError {
    Read((std::io::Error, String)),
    Compile {
        error: wgsl::ParseError,
        source: String,
    },
    Validate(naga::WithSpan<ValidationError>),
    WriteSpirv(spv::Error),
}

impl From<naga::WithSpan<ValidationError>> for CompilerError {
    fn from(e: naga::WithSpan<ValidationError>) -> Self {
        Self::Validate(e)
    }
}

impl From<spv::Error> for CompilerError {
    fn from(e: spv::Error) -> Self {
        Self::WriteSpirv(e)
    }
}

impl std::fmt::Display for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read((err, path)) => write!(f, "{} with path: {}", err, path),
            Self::WriteSpirv(err) => write!(f, "{}", err),
            Self::Validate(err) => write!(f, "{}", err),
            Self::Compile { error, source } => {
                error.emit_to_stderr(source);
                Ok(())
            }
        }
    }
}

impl std::fmt::Debug for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read((err, path)) => write!(f, "{} with path: {path}", err),
            Self::WriteSpirv(err) => write!(f, "{}", err),
            Self::Validate(err) => write!(f, "{}", err),
            Self::Compile { error, source } => write!(f, "{}", error.emit_to_string(source)),
        }
    }
}

impl std::error::Error for CompilerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Self::Read((ref e, _)) => Some(e),
            Self::Compile { error: ref e, .. } => Some(e),
            Self::Validate(ref e) => Some(e),
            Self::WriteSpirv(ref e) => Some(e),
        }
    }
}
