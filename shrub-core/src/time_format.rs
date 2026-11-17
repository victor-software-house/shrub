//! Formatting times with `strftime` conversions, as the prompt's `\D{format}`
//! and `HISTTIMEFORMAT` take them.

use jiff::fmt::strtime::{BrokenDownTime, Config, PosixCustom};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

use crate::error;

/// Formats `timestamp` in the system time zone with the `strftime`
/// conversions in `format`.
///
/// Conversions print as in the POSIX locale. A conversion that is unknown, or
/// that the time cannot fill, prints as written, as glibc's `strftime` leaves
/// it.
///
/// # Errors
///
/// Returns [`error::ErrorKind::TimeFormatError`] if writing the formatted
/// text fails.
pub fn format_local(timestamp: Timestamp, format: &str) -> Result<String, error::Error> {
    format_zoned(&timestamp.to_zoned(TimeZone::system()), format)
}

/// Formats `zoned` with the `strftime` conversions in `format`, as
/// [`format_local`] does.
pub(crate) fn format_zoned(zoned: &Zoned, format: &str) -> Result<String, error::Error> {
    let config = Config::new().custom(PosixCustom::new()).lenient(true);
    Ok(BrokenDownTime::from(zoned).to_string_with_config(&config, format)?)
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "assertions in fallible tests")]
mod tests {
    use super::*;

    fn christmas() -> Result<Zoned, error::Error> {
        let timestamp: Timestamp = "2024-12-25T13:34:56.789Z".parse()?;
        Ok(timestamp.to_zoned(TimeZone::UTC))
    }

    /// Conversions print in the POSIX locale.
    #[test]
    fn formats_in_the_posix_locale() -> Result<(), error::Error> {
        assert_eq!(
            format_zoned(&christmas()?, "%a %b %d %Y-%m-%d %H:%M:%S")?,
            "Wed Dec 25 2024-12-25 13:34:56"
        );
        assert_eq!(
            format_zoned(&christmas()?, "%c")?,
            "Wed Dec 25 13:34:56 2024"
        );
        Ok(())
    }

    /// An unknown conversion prints as written, and the rest still formats.
    #[test]
    fn unknown_conversion_prints_as_written() -> Result<(), error::Error> {
        assert_eq!(format_zoned(&christmas()?, "%Y %! %")?, "2024 %! %");
        Ok(())
    }
}
