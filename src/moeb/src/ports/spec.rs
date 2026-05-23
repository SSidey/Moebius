use anyhow::Result;

use crate::trace::FileContentMode;

/// Primary port — driven by the CLI adapter to produce a specification from raw input.
pub trait SpecPort {
    fn run(&self, input: &str, file_content_mode: FileContentMode, no_review: bool) -> Result<()>;
}
