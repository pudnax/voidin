use std::path::Path;

use color_eyre::Result;
use naga::{
    back::spv::{self, BindingMap},
    front::wgsl,
    valid::{Capabilities, ValidationError, ValidationFlags, Validator},
};

const SHADER_FOLDER: &str = "shaders";

pub struct ShaderCompiler {
    parser: wgsl::Frontend,
    validator: Validator,
    writer: spv::Writer,
}

impl ShaderCompiler {
    pub fn new() -> Self {
        let parser = wgsl::Frontend::new();
        let validator = Validator::new(ValidationFlags::all(), Capabilities::all());
        let options = get_options();
        let writer = spv::Writer::new(&options).unwrap();
        Self {
            parser,
            validator,
            writer,
        }
    }

    pub fn create_shader_module(&mut self, path: &Path) -> Result<Vec<u32>, CompilerError> {
        let parser = ocl_include::Parser::builder()
            .add_source(
                ocl_include::source::Fs::builder()
                    .include_dir(uni_path::Path::new(SHADER_FOLDER))
                    .map_err(|err| CompilerError::Include((err, path.to_string_lossy().into())))?
                    .build(),
            )
            .build();

        let parsed_res = parser
            .parse(uni_path::Path::new(path.to_str().unwrap()))
            .map_err(|err| CompilerError::Read((err, path.to_string_lossy().into())))?;
        let source = parsed_res.collect().0;
        let module = self
            .parser
            .parse(&source)
            .map_err(|error| CompilerError::Compile {
                error,
                source: source.clone(),
                path: path.display().to_string(),
            })?;
        let module_info =
            self.validator
                .validate(&module)
                .map_err(|error| CompilerError::Validate {
                    error: Box::new(error),
                    source,
                    path: path.display().to_string(),
                })?;
        let mut words = vec![];
        self.writer.write(&module, &module_info, None, &mut words)?;
        Ok(words)
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

impl Default for ShaderCompiler {
    fn default() -> Self {
        Self::new()
    }
}

pub enum CompilerError {
    Include((std::io::Error, String)),
    Read((std::io::Error, String)),
    Compile {
        error: wgsl::ParseError,
        source: String,
        path: String,
    },
    Validate {
        error: Box<naga::WithSpan<ValidationError>>,
        source: String,
        path: String,
    },
    WriteSpirv(spv::Error),
}

impl From<spv::Error> for CompilerError {
    fn from(e: spv::Error) -> Self {
        Self::WriteSpirv(e)
    }
}

impl std::fmt::Display for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::fmt::Debug for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Include((err, path)) => {
                write!(f, "Failed resolve include\n{} with path: {}", err, path)
            }
            Self::Read((err, path)) => write!(f, "{} with path: {path}", err),
            Self::WriteSpirv(err) => write!(f, "{}", err),
            Self::Compile {
                error,
                source,
                path,
            } => {
                write!(f, "{}", error.emit_to_string_with_path(source, path))
            }
            Self::Validate {
                error,
                source,
                path,
            } => {
                write!(f, "{}", error.emit_to_string_with_path(source, path))
            }
        }
    }
}

impl std::error::Error for CompilerError {}
