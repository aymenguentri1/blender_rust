//! Port of `source/blender/blenlib/intern/math_time.cc` (`BLI_math_time_seconds_decompose`) and
//! the time constants of `BLI_math_time.hh`.
//! Tests are the `SecondsExplode` case of `tests/BLI_math_time_test.cc`.

pub const SECONDS_IN_MILLISECONDS: f64 = 0.001;
pub const SECONDS_IN_MINUTE: f64 = 60.0;
pub const MINUTES_IN_HOUR: f64 = 60.0;
pub const HOURS_IN_DAY: f64 = 24.0;

pub const MINUTES_IN_DAY: f64 = MINUTES_IN_HOUR * HOURS_IN_DAY;
pub const SECONDS_IN_DAY: f64 = MINUTES_IN_DAY * SECONDS_IN_MINUTE;
pub const SECONDS_IN_HOUR: f64 = MINUTES_IN_HOUR * SECONDS_IN_MINUTE;

/// `BLI_math_time_seconds_decompose()`: splits `seconds` into whole days, hours, minutes and
/// seconds and milliseconds. Only the requested (`Some`) outputs are produced; the remainder
/// goes into the smallest requested unit. At least one output must be requested.
pub fn seconds_decompose(
    mut seconds: f64,
    r_days: Option<&mut f64>,
    r_hours: Option<&mut f64>,
    r_minutes: Option<&mut f64>,
    r_seconds: Option<&mut f64>,
    r_milliseconds: Option<&mut f64>,
) {
    debug_assert!(
        r_days.is_some()
            || r_hours.is_some()
            || r_minutes.is_some()
            || r_seconds.is_some()
            || r_milliseconds.is_some()
    );
    let (mut r_days, mut r_hours, mut r_minutes, mut r_seconds) = (r_days, r_hours, r_minutes, r_seconds);

    /* `modf(x, &int_part)` is `x.fract()` with the integer part in `x.trunc()`. */
    if let Some(days) = r_days.as_deref_mut() {
        let x = seconds / SECONDS_IN_DAY;
        *days = x.trunc();
        seconds = x.fract() * SECONDS_IN_DAY;
    }
    if let Some(hours) = r_hours.as_deref_mut() {
        let x = seconds / SECONDS_IN_HOUR;
        *hours = x.trunc();
        seconds = x.fract() * SECONDS_IN_HOUR;
    }
    if let Some(minutes) = r_minutes.as_deref_mut() {
        let x = seconds / SECONDS_IN_MINUTE;
        *minutes = x.trunc();
        seconds = x.fract() * SECONDS_IN_MINUTE;
    }
    if let Some(secs) = r_seconds.as_deref_mut() {
        *secs = seconds.trunc();
        seconds = seconds.fract();
    }

    if let Some(ms) = r_milliseconds {
        *ms = seconds / SECONDS_IN_MILLISECONDS;
    } else if let Some(secs) = r_seconds {
        *secs += seconds;
    } else if let Some(minutes) = r_minutes {
        *minutes += seconds / SECONDS_IN_MINUTE;
    } else if let Some(hours) = r_hours {
        *hours += seconds / SECONDS_IN_HOUR;
    } else if let Some(days) = r_days {
        *days = seconds / SECONDS_IN_DAY;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_explode() {
        let seconds = 2.0 * SECONDS_IN_DAY
            + 13.0 * SECONDS_IN_HOUR
            + 33.0 * SECONDS_IN_MINUTE
            + 9.0
            + 369.0 * SECONDS_IN_MILLISECONDS;
        let epsilon = 1e-8;
        let near = |a: f64, b: f64| (a - b).abs() <= epsilon;

        let (mut days, mut hours, mut minutes, mut secs, mut millis) = (0.0, 0.0, 0.0, 0.0, 0.0);

        seconds_decompose(seconds, Some(&mut days), Some(&mut hours), Some(&mut minutes), Some(&mut secs), Some(&mut millis));
        assert!(near(2.0, days));
        assert!(near(13.0, hours));
        assert!(near(33.0, minutes));
        assert!(near(9.0, secs));
        assert!(near(369.0, millis));

        seconds_decompose(seconds, None, Some(&mut hours), Some(&mut minutes), Some(&mut secs), None);
        assert!(near(61.0, hours));
        assert!(near(33.0, minutes));
        assert!(near(9.369, secs));

        seconds_decompose(seconds, None, None, None, Some(&mut secs), None);
        assert!(near(seconds, secs));

        seconds_decompose(seconds, Some(&mut days), None, Some(&mut minutes), None, Some(&mut millis));
        assert!(near(2.0, days));
        assert!(near(813.0, minutes));
        assert!(near(9369.0, millis));
    }
}
