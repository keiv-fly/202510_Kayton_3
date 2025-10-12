pub mod ast;
pub mod diagnostics;
pub mod hir;
pub mod interner;
pub mod lexer;
pub mod lowering;
pub mod parser;
pub mod source;
pub mod span;

use std::path::Path;

use diagnostics::Diagnostic;
use hir::HirModule;
use source::SourceMap;

#[derive(Debug)]
pub struct ParseOutput {
    pub module: HirModule,
    pub diagnostics: Vec<Diagnostic>,
    pub source_map: SourceMap,
}

#[derive(thiserror::Error, Debug)]
pub enum FrontendError {
    #[error("failed to read source: {0}")]
    Io(#[from] std::io::Error),
}

pub fn parse_to_hir(path: &Path) -> Result<ParseOutput, FrontendError> {
    let mut source_map = SourceMap::new();
    let text = std::fs::read_to_string(path)?;
    let source_id = source_map.add_source(path.to_path_buf(), text.clone());
    let mut diagnostics = Vec::new();
    let (tokens, mut lex_diags) = lexer::lex(&text, source_id);
    diagnostics.append(&mut lex_diags);
    let mut parser = parser::Parser::new(tokens, source_id);
    let ast_module = parser.parse_module();
    diagnostics.extend(parser.into_diagnostics());
    let mut hir_builder = lowering::LoweringContext::new(source_map.clone());
    let module = hir_builder.lower_module(ast_module);
    diagnostics.extend(hir_builder.into_diagnostics());
    Ok(ParseOutput {
        module,
        diagnostics,
        source_map,
    })
}

pub mod tests_support {
    use super::*;
    use std::path::PathBuf;

    pub fn parse_str(name: &str, source: &str) -> ParseOutput {
        let mut source_map = SourceMap::new();
        let source_id = source_map.add_source(PathBuf::from(name), source.to_string());
        let (tokens, mut lex_diags) = crate::lexer::lex(source, source_id);
        let mut diagnostics = Vec::new();
        diagnostics.append(&mut lex_diags);
        let mut parser = crate::parser::Parser::new(tokens, source_id);
        let ast_module = parser.parse_module();
        diagnostics.extend(parser.into_diagnostics());
        let mut lowering = crate::lowering::LoweringContext::new(source_map.clone());
        let module = lowering.lower_module(ast_module);
        diagnostics.extend(lowering.into_diagnostics());
        ParseOutput {
            module,
            diagnostics,
            source_map,
        }
    }
}
