use anyhow::Result;

use crate::cli::Config;
use crate::clipboard::write_clipboard;
use crate::logging::log_info;

pub fn output_text(text: &str, config: &Config) -> Result<()> {
    if config.use_clipboard_output {
        log_info("Writing response to clipboard");
        write_clipboard(text)
    } else {
        log_info("Writing response to stdout");
        print!("{}", text);
        Ok(())
    }
}
