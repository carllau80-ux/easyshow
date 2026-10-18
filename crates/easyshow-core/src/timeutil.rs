//! 时间工具:EXIF 拍摄时间解析、系统时间 → 本地年月日时分秒。
//!
//! EXIF 的 DateTimeOriginal 本身就是**本地墙钟时间**,直接解析成
//! [`DateTimeParts`] 即可用于排序与按年/月/日分组;文件时间则通过
//! 平台 localtime 转成本地墙钟,保证时间轴分组符合用户直觉。

use std::time::{SystemTime, UNIX_EPOCH};

/// 本地墙钟时间(年月日时分秒)。按字段顺序实现 Ord,可直接排序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DateTimeParts {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl DateTimeParts {
    pub fn is_valid(&self) -> bool {
        (1900..=3000).contains(&self.year)
            && (1..=12).contains(&self.month)
            && (1..=31).contains(&self.day)
    }
}

/// 解析 EXIF 日期字符串,如 `2026:05:18 19:42:05`。
pub fn parse_exif(text: &str) -> Option<DateTimeParts> {
    let text = text.trim().trim_matches('"');
    let (date, time) = text.split_once(' ')?;
    // EXIF 原始写法是 "2026:05:18",而 kamadak-exif 的显示值是
    // "2026-05-18";两种分隔符都接受。
    let sep = if date.contains('-') { '-' } else { ':' };
    let mut d = date.split(sep);
    let year: i32 = d.next()?.trim().parse().ok()?;
    let month: u8 = d.next()?.trim().parse().ok()?;
    let day: u8 = d.next()?.trim().parse().ok()?;
    let mut t = time.split(':');
    let hour: u8 = t.next()?.trim().parse().ok()?;
    let minute: u8 = t.next()?.trim().parse().ok()?;
    let second: u8 = t.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let parts = DateTimeParts { year, month, day, hour, minute, second };
    parts.is_valid().then_some(parts)
}

/// 系统时间 → 本地墙钟时分秒。
pub fn from_system_time_local(t: SystemTime) -> DateTimeParts {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    from_unix_local(secs)
}

#[cfg(unix)]
fn from_unix_local(secs: i64) -> DateTimeParts {
    unsafe {
        let t = secs as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return DateTimeParts::default();
        }
        DateTimeParts {
            year: tm.tm_year + 1900,
            month: (tm.tm_mon + 1).clamp(1, 12) as u8,
            day: tm.tm_mday.clamp(1, 31) as u8,
            hour: tm.tm_hour.clamp(0, 23) as u8,
            minute: tm.tm_min.clamp(0, 59) as u8,
            second: tm.tm_sec.clamp(0, 59) as u8,
        }
    }
}

#[cfg(windows)]
fn from_unix_local(secs: i64) -> DateTimeParts {
    use std::ptr;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    extern "system" {
        fn FileTimeToSystemTime(ft: *const FileTime, st: *mut SystemTime) -> i32;
        // 传入 NULL 时区使用当前系统时区,并按该日期正确处理夏令时。
        fn SystemTimeToTzSpecificLocalTime(
            tz: *const std::ffi::c_void,
            utc: *const SystemTime,
            local: *mut SystemTime,
        ) -> i32;
    }

    // 1601-01-01 到 1970-01-01 的秒数;FILETIME 每 100ns 一个 tick。
    const EPOCH_DIFF: i64 = 11_644_473_600;
    let Some(ticks) = secs
        .checked_add(EPOCH_DIFF)
        .and_then(|s| s.checked_mul(10_000_000))
    else {
        return DateTimeParts::default();
    };
    let ft = FileTime { low: ticks as u32, high: (ticks >> 32) as u32 };
    unsafe {
        let mut utc = SystemTime::default();
        let mut local = SystemTime::default();
        if FileTimeToSystemTime(&ft, &mut utc) == 0 {
            return DateTimeParts::default();
        }
        if SystemTimeToTzSpecificLocalTime(ptr::null(), &utc, &mut local) == 0 {
            return DateTimeParts::default();
        }
        DateTimeParts {
            year: local.year as i32,
            month: local.month.clamp(1, 12) as u8,
            day: local.day.clamp(1, 31) as u8,
            hour: local.hour.clamp(0, 23) as u8,
            minute: local.minute.clamp(0, 59) as u8,
            second: local.second.clamp(0, 59) as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exif_datetime() {
        let t = parse_exif("2026:05:18 19:42:05").unwrap();
        assert_eq!((t.year, t.month, t.day), (2026, 5, 18));
        assert_eq!((t.hour, t.minute, t.second), (19, 42, 5));
        assert!(parse_exif("not a date").is_none());
        assert!(parse_exif("2026:13:01 00:00:00").is_none());
        // 个别相机把时间写成 "2026:05:18 19:42" 或带空格
        assert!(parse_exif("2026:05:18 19:42").is_some());
        // kamadak-exif 的 display_value 使用短横线日期
        let dashed = parse_exif("2026-05-18 19:42:05").unwrap();
        assert_eq!((dashed.year, dashed.month, dashed.day), (2026, 5, 18));
    }

    #[test]
    fn system_time_converts_to_local_parts() {
        let p = from_system_time_local(SystemTime::UNIX_EPOCH);
        // 1970-01-01 本地时间(东八区为 08:00)
        assert_eq!(p.year, 1970);
        assert_eq!(p.month, 1);
        assert_eq!(p.day, 1);
    }
}
