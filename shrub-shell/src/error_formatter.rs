#[derive(Debug, Default, Clone)]
pub(crate) struct Formatter {
    pub use_color: bool,
}

impl shrub_core::extensions::ErrorFormatter for Formatter {
    fn format_error(
        &self,
        err: &shrub_core::error::Error,
        _shell: &shrub_core::Shell<impl shrub_core::ShellExtensions>,
    ) -> String {
        let prefix = if self.use_color {
            color_print::cstr!("<red>error:</red> ")
        } else {
            "error: "
        };

        std::format!("{prefix}{err:#}\n")
    }
}
