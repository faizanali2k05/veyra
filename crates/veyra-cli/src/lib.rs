use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};
use veyra_ast::{Decl, Program};
use veyra_interpreter::evaluate;
use veyra_lexer::lex;
use veyra_parser::{parse_expression, parse_program};
use veyra_source::{Source, Span};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineError {
    pub message: String,
    pub span: Span,
}
#[must_use]
pub fn render_error(source: &Source, error: &PipelineError) -> String {
    source.render(&error.message, error.span)
}
/// Runs a standalone integer expression through the frontend and evaluator.
/// # Errors
/// Returns the first lexical, parse, or runtime error.
pub fn eval(source: &Source) -> Result<i64, PipelineError> {
    let tokens = lex(source.text()).map_err(|e| PipelineError {
        message: e.message,
        span: e.span,
    })?;
    let expression = parse_expression(tokens).map_err(|e| PipelineError {
        message: e.message,
        span: e.span,
    })?;
    evaluate(&expression).map_err(|e| PipelineError {
        message: e.message,
        span: e.span,
    })
}

pub struct Project {
    pub root: Program,
    pub modules: HashMap<String, Program>,
    pub root_source: Source,
    pub module_sources: HashMap<String, Source>,
}
/// Loads and parses a source file and its sibling module imports.
/// # Errors
/// Returns a rendered diagnostic or I/O error without panicking on user input.
pub fn load_project(path: &Path) -> Result<Project, String> {
    let root_text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let root_source = Source::new(path.display().to_string(), root_text);
    let root = parse(&root_source)?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let mut queue: VecDeque<String> = imports(&root).into();
    let mut seen = HashSet::new();
    let mut modules = HashMap::new();
    let mut module_sources = HashMap::new();
    while let Some(name) = queue.pop_front() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let module_path = directory.join(format!("{name}.vy"));
        let text = fs::read_to_string(&module_path).map_err(|e| {
            format!(
                "cannot load module `{name}` from {}: {e}",
                module_path.display()
            )
        })?;
        let source = Source::new(module_path.display().to_string(), text);
        let program = parse(&source)?;
        for import in imports(&program) {
            queue.push_back(import);
        }
        modules.insert(name.clone(), program);
        module_sources.insert(name, source);
    }
    Ok(Project {
        root,
        modules,
        root_source,
        module_sources,
    })
}
fn parse(source: &Source) -> Result<Program, String> {
    let tokens = lex(source.text()).map_err(|e| source.render(&e.message, e.span))?;
    parse_program(tokens).map_err(|e| source.render(&e.message, e.span))
}
fn imports(program: &Program) -> Vec<String> {
    program
        .declarations
        .iter()
        .filter_map(|d| {
            if let Decl::Import(i) = d {
                Some(i.name.clone())
            } else {
                None
            }
        })
        .collect()
}
#[must_use]
pub fn default_build_path(source: &Path) -> PathBuf {
    source.with_extension("vyb")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expression_pipeline() {
        assert_eq!(eval(&Source::new("<test>", "2 + 3 * (4 - 1)")).unwrap(), 11);
    }
}
