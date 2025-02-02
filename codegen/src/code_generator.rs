use std::{
    error, fmt,
    fs::File,
    io::{self, BufWriter},
};

use crate::command_map::CommandMap;

mod languages;

/* * * * Public interface * * * */

#[derive(Debug)]
pub enum GeneratorError {
    WriteError(io::Error),
}

pub struct CodeGenerator;

impl CodeGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn write_typescript(
        &self,
        path: String,
        commands: &CommandMap,
    ) -> Result<(), GeneratorError> {
        let file: File = File::create(path)?;
        let mut writer: BufWriter<File> = BufWriter::new(file);
        languages::typescript::write_to_typescript(commands, &mut writer)?;
        Ok(())
    }
}

/* * * * Private implementation * * * */

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteError(e) => {
                write!(f, "failed to write generated code: {e}")
            }
        }
    }
}

impl error::Error for GeneratorError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::WriteError(e) => Some(e),
        }
    }
}

impl From<io::Error> for GeneratorError {
    fn from(value: io::Error) -> Self {
        Self::WriteError(value)
    }
}
