//! Invocation contract for the viewer launcher.

use std::{ffi::OsString, time::Duration};

pub(crate) fn duration(args: &[OsString]) -> Result<Option<Duration>, &'static str> {
    match args {
        [] => Ok(None),
        [flag, value] if flag == "--for-seconds" => value
            .to_str()
            .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse().ok())
            .filter(|n| (1..=900).contains(n))
            .map(Duration::from_secs)
            .map(Some)
            .ok_or("--for-seconds requires a whole number from 1 to 900"),
        _ => Err("expected no viewer arguments or --for-seconds N (1 to 900)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_default_and_inclusive_deadlines() {
        assert_eq!(duration(&[]), Ok(None));
        for seconds in [1, 900] {
            assert_eq!(
                duration(&["--for-seconds".into(), seconds.to_string().into()]),
                Ok(Some(Duration::from_secs(seconds)))
            );
        }
    }
}
