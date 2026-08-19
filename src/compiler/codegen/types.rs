use std::collections::HashMap;

use crate::{compiler::{obfuscation::ObfuscatedExpr}, core::error::CompileError};

pub struct JsBufferBuilder {
    pub var_zone: String,
    pub binding_zone: String,
    pub component_function_zone: String,
    pub component_function_registry: HashMap<String, String>,
}

pub struct OutputBuffer {
    pub js: JsBufferBuilder,
    pub html: String,
}

pub struct Compiler {
    pub buffer: OutputBuffer,
    pub options: CompilerOptions,
    pub scope_id: Option<ObfuscatedExpr>
}

#[derive(Debug, Clone, Copy)]
pub struct CompilerOptions{
    pub codegen_strategy: CodegenStrategy
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenStrategy {
    AsIs,
    MinifyJsOnly,
    MinifyAll
}



impl Default for CompilerOptions {
    fn default() -> Self {
        Self { codegen_strategy: CodegenStrategy::MinifyAll }
    }
}

impl TryFrom<usize> for CodegenStrategy {
    type Error = CompileError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::AsIs),
            1 => Ok(Self::MinifyJsOnly),
            2 => Ok(Self::MinifyAll),
            _ => Err(CompileError::plain("Invalid minify level."))
        }
    }
}


impl Compiler {
    pub fn new(options: CompilerOptions) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            options,
            scope_id: None,
        }
    }
}

impl OutputBuffer {
    pub fn new() -> Self {
        Self {
            js: JsBufferBuilder::new(),
            html: String::new(),
        }
    }
}

impl JsBufferBuilder {
    fn new() -> Self {
        Self {
            var_zone: String::new(),
            binding_zone: String::new(),
            component_function_zone: String::new(),
            component_function_registry: HashMap::new(),
        }
    }
}