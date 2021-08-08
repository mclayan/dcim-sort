use crate::pattern::PatternElement;
use chrono::{DateTime, Local, Datelike, Timelike};
use crate::media::ImgInfo;
use crate::pattern::device::DevicePart;

pub struct ScreenshotPattern {
    segment_name: String,
}

impl ScreenshotPattern {
    pub fn def_value() -> String {
        String::from("screenshots")
    }

    pub fn new(seg_name: String) -> Box<dyn PatternElement> {

        Box::new(ScreenshotPattern {
            segment_name: seg_name
        })
    }
}
impl PatternElement for ScreenshotPattern {

    fn is_optional(&self) -> bool {
        true
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        let m = info.metadata();
        if m.is_screenshot() {
            Some(self.segment_name.clone())
        }
        else {
            None
        }
    }
}


pub enum DateTimePart {
    /// Year, formatted as 'YYYY'
    Year,
    /// Month, formatted as 'mm'
    Month,
    /// Day, formatted as 'DD'
    Day,
    /// Hour, formatted as 'hh' in 24-hour format
    Hour,
    /// Minute, formatted as 'mm'
    Minute,
    /// Second, formatted as 'ss'
    Second
}

impl DateTimePart {
    pub fn parse(s: &str) -> Option<DateTimePart> {
        match s.to_lowercase().as_str() {
            "year"   => Some(DateTimePart::Year),
            "month"  => Some(DateTimePart::Month),
            "day"    => Some(DateTimePart::Day),
            "hour"   => Some(DateTimePart::Hour),
            "minute" => Some(DateTimePart::Minute),
            "second" => Some(DateTimePart::Second),
            _        => None
        }
    }
}

/// Pattern to generate a segment based on a timestamp
/// associated with the file. Can be configured via
/// separators. Values are always expanded to fixed-
/// width strings and padded with '0'.
pub struct DateTimePattern {
    fs_timestamp_fallback: bool,
    separator: char,
    default: String,
    pattern: Vec<DateTimePart>
}
pub struct DateTimePatternBuilder {
    fs_timestamp_fallback: bool,
    separator: char,
    default: String,
    pattern: Vec<DateTimePart>
}

impl DateTimePattern {
    pub fn def_fs_timestamp_fallback() -> bool {
        false
    }

    pub fn def_separator() -> char {
        '-'
    }

    pub fn def_default() -> String {
        String::from("unknown")
    }

    pub fn new() -> DateTimePatternBuilder {
        DateTimePatternBuilder {
            fs_timestamp_fallback: Self::def_fs_timestamp_fallback(),
            separator: Self::def_separator(),
            default: Self::def_default(),
            pattern: Vec::new()
        }
    }
    fn generate_result(&self, ts: &DateTime<Local>) -> String {
        let mut result = String::new();
        let mut first = true;
        for part in &self.pattern {
            if first {
                first = false;
            }
            else {
                result.push(self.separator);
            }
            match part {
                DateTimePart::Year => result.push_str(format!("{:04}", ts.year()).as_str()),
                DateTimePart::Month => result.push_str(format!("{:02}", ts.month()).as_str()),
                DateTimePart::Day => result.push_str(format!("{:02}", ts.day()).as_str()),
                DateTimePart::Hour => result.push_str(format!("{:02}", ts.hour()).as_str()),
                DateTimePart::Minute => result.push_str(format!("{:02}", ts.minute()).as_str()),
                DateTimePart::Second => result.push_str(format!("{:02}", ts.second()).as_str()),
            }
        }
        result
    }
}
impl PatternElement for DateTimePattern {
    fn is_optional(&self) -> bool {
        false
    }

    fn translate(&self, info: &ImgInfo) -> Option<String> {
        let timestamp : Option<&DateTime<Local>> = match info.metadata().created_at() {
            Some(ts) => Some(ts),
            None => {
                if self.fs_timestamp_fallback {
                    Some(info.changed_at())
                }
                else {
                    None
                }
            }
        };
        let result = match timestamp {
            Some(ts) => self.generate_result(ts),
            None => self.default.clone()
        };
        Some(result)
    }
}
impl DateTimePatternBuilder {
    pub fn part(mut self, p: DateTimePart) -> DateTimePatternBuilder {
        self.pattern.push(p);
        self
    }

    pub fn separator(mut self, s: char) -> DateTimePatternBuilder {
        self.separator = s;
        self
    }

    pub fn default(mut self, s: String) -> DateTimePatternBuilder {
        self.default = s;
        self
    }

    pub fn fs_timestamp_fallback(mut self, b: bool) -> DateTimePatternBuilder {
        self.fs_timestamp_fallback = b;
        self
    }

    pub fn push_part(&mut self, part: DateTimePart) {
        self.pattern.push(part);
    }

    pub fn build(mut self) -> Box<dyn PatternElement> {
        if self.pattern.len() == 0 {
            self.pattern = vec![DateTimePart::Year, DateTimePart::Month]
        }
        Box::new(DateTimePattern{
            fs_timestamp_fallback: self.fs_timestamp_fallback,
            separator: self.separator,
            default: self.default,
            pattern: self.pattern
        })
    }
}